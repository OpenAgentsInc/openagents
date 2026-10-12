// Standalone check of psionic_clef_hopper_linear (clef_hopper.cu, #11443)
// on random Q8_0 / Q4_K weights at the Clef-Flash and Clef 27B trunk
// shapes: error vs an f32-accumulate cuBLAS reference on the same
// dequantized f16 weights, bitwise chunk invariance (64-row chunks and every
// token tile against the whole), residual accumulation, and timing against
// today's paths (the fused mma.sync kernel, dequantize + cuBLAS f16, and
// cuBLAS on resident f16 weights). Build on an H100:
//
//   nvcc -std=c++17 -O3 -gencode arch=compute_90a,code=sm_90a \
//        -I<psionic-backend-cuda>/src/kernels hopper_gemm_bench.cu -lcublas -o hopper_gemm_bench
//   ./hopper_gemm_bench [iters] [flash|27b|all] [n ...]
#include "clef_prefill.cu"
#include "clef_hopper.cu"

#include <cublas_v2.h>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <random>
#include <string>
#include <vector>

#define CK(x)                                                                    \
    do {                                                                         \
        auto e = (x);                                                            \
        if (e != 0) {                                                            \
            fprintf(stderr, "%s:%d %s -> %d\n", __FILE__, __LINE__, #x, (int)e); \
            exit(1);                                                             \
        }                                                                        \
    } while (0)

static uint16_t f2h(float f) {
    __half h = __float2half(f);
    uint16_t b;
    memcpy(&b, &h, 2);
    return b;
}

static std::vector<uint8_t> random_weights(int format, int rows, int k, std::mt19937 &rng) {
    std::uniform_int_distribution<int> byte(0, 255);
    std::uniform_real_distribution<float> scale(0.002f, 0.02f);
    if (format == 0) {
        std::vector<uint8_t> w((size_t)rows * k / 32 * 34);
        for (size_t b = 0; b < w.size() / 34; ++b) {
            uint16_t d = f2h(scale(rng) * 0.2f);
            memcpy(&w[b * 34], &d, 2);
            for (int i = 0; i < 32; ++i) w[b * 34 + 2 + i] = (uint8_t)byte(rng);
        }
        return w;
    }
    std::vector<uint8_t> w((size_t)rows * k / 256 * 144);
    for (size_t b = 0; b < w.size() / 144; ++b) {
        uint16_t d = f2h(scale(rng) * 0.1f), dmin = f2h(scale(rng) * 0.1f);
        memcpy(&w[b * 144], &d, 2);
        memcpy(&w[b * 144 + 2], &dmin, 2);
        for (int i = 4; i < 144; ++i) w[b * 144 + i] = (uint8_t)byte(rng);
    }
    return w;
}

static double peak_tf = 989.4;  // H100 SXM dense f16

int main(int argc, char **argv) {
    int iters = argc > 1 ? atoi(argv[1]) : 20;
    std::string set = argc > 2 ? argv[2] : "all";
    struct Shape {
        const char *model, *name;
        int format, rows, k;
    } all_shapes[] = {
        {"flash", "ffn_gate/up Q4_K 12288x4096", 1, 12288, 4096},
        {"flash", "ffn_down Q4_K 4096x12288", 1, 4096, 12288},
        {"flash", "attn_qkv/q Q8_0 8192x4096", 0, 8192, 4096},
        {"flash", "ssm_out/gate Q8_0 4096x4096", 0, 4096, 4096},
        {"flash", "attn_k/v Q8_0 1024x4096", 0, 1024, 4096},
        {"flash", "alpha/beta Q8_0 32x4096", 0, 32, 4096},
        {"27b", "ffn_gate/up Q4_K 17408x5120", 1, 17408, 5120},
        {"27b", "ffn_down Q4_K 5120x17408", 1, 5120, 17408},
        {"27b", "attn_q Q8_0 12288x5120", 0, 12288, 5120},
        {"27b", "attn_qkv Q8_0 10240x5120", 0, 10240, 5120},
        {"27b", "attn_gate Q8_0 6144x5120", 0, 6144, 5120},
        {"27b", "ssm_out/attn_out Q8_0 5120x6144", 0, 5120, 6144},
        {"27b", "attn_k/v Q8_0 1024x5120", 0, 1024, 5120},
        {"27b", "alpha/beta Q8_0 48x5120", 0, 48, 5120},
    };
    std::vector<int> ns;
    for (int i = 3; i < argc; ++i) ns.push_back(atoi(argv[i]));
    if (ns.empty()) ns = {1, 17, 100, 1082, 2048, 3917};
    if (!psionic_clef_hopper_supported()) {
        fprintf(stderr, "hopper kernels not supported on this device/build\n");
        return 2;
    }
    std::mt19937 rng(7);
    cublasHandle_t handle;
    CK(cublasCreate(&handle));
    cudaStream_t stream;
    CK(cudaStreamCreate(&stream));
    CK(cublasSetStream(handle, stream));
    cudaEvent_t e0, e1;
    cudaEventCreate(&e0);
    cudaEventCreate(&e1);
    int nmax = 0;
    for (int n : ns) nmax = n > nmax ? n : nmax;
    bool all_ok = true;
    for (auto &s : all_shapes) {
        if (set != "all" && set != s.model) continue;
        auto host_w = random_weights(s.format, s.rows, s.k, rng);
        std::normal_distribution<float> normal(0.0f, 1.0f);
        std::vector<__half> host_x((size_t)nmax * s.k);
        for (auto &v : host_x) v = __float2half(normal(rng));
        void *dw, *dx, *dw16, *dref, *dout, *dout2, *d16;
        CK(cudaMalloc(&dw, host_w.size()));
        CK(cudaMalloc(&dx, host_x.size() * 2));
        CK(cudaMalloc(&dw16, (size_t)s.rows * s.k * 2));
        CK(cudaMalloc(&dref, (size_t)nmax * s.rows * 4));
        CK(cudaMalloc(&dout, (size_t)nmax * s.rows * 4));
        CK(cudaMalloc(&dout2, (size_t)nmax * s.rows * 4));
        CK(cudaMalloc(&d16, (size_t)nmax * s.rows * 2));
        CK(cudaMemcpy(dw, host_w.data(), host_w.size(), cudaMemcpyHostToDevice));
        CK(cudaMemcpy(dx, host_x.data(), host_x.size() * 2, cudaMemcpyHostToDevice));
        long long elements = (long long)s.rows * s.k;
        auto dequant = [&]() {
            if (s.format == 0)
                CK(psionic_clef_dequant_q8_0_f16(dw, dw16, elements / 32, stream));
            else
                CK(psionic_clef_dequant_q4_k_f16(dw, dw16, elements / 256, stream));
        };
        dequant();
        auto time_ms = [&](auto &&fn) {
            fn();
            cudaEventRecord(e0, stream);
            for (int it = 0; it < iters; ++it) fn();
            cudaEventRecord(e1, stream);
            cudaEventSynchronize(e1);
            float ms;
            cudaEventElapsedTime(&ms, e0, e1);
            return ms / iters;
        };
        // HOPPER_EXACT=1: Q4_K through the exact f32 dequantization (format 2)
        const int hfmt = s.format == 1 && getenv("HOPPER_EXACT") ? 2 : s.format;
        for (int n : ns) {
            float one = 1.0f, zero = 0.0f;
            double flop = 2.0 * n * s.rows * s.k;
            CK(cublasGemmEx(handle, CUBLAS_OP_T, CUBLAS_OP_N, s.rows, n, s.k, &one, dw16, CUDA_R_16F, s.k, dx,
                            CUDA_R_16F, s.k, &zero, dref, CUDA_R_32F, s.rows, CUBLAS_COMPUTE_32F,
                            CUBLAS_GEMM_DEFAULT));
            std::vector<float> ref((size_t)n * s.rows), got((size_t)n * s.rows), got2((size_t)n * s.rows);
            CK(cudaMemcpy(ref.data(), dref, ref.size() * 4, cudaMemcpyDeviceToHost));
            double ref_rms = 0;
            for (float v : ref) ref_rms += (double)v * v;
            ref_rms = sqrt(ref_rms / ref.size());
            auto errors = [&](const std::vector<float> &v, double &max_err, double &rel_rms) {
                double sq = 0;
                max_err = 0;
                for (size_t i = 0; i < v.size(); ++i) {
                    double e = fabs((double)v[i] - ref[i]);
                    max_err = fmax(max_err, e);
                    sq += e * e;
                }
                rel_rms = sqrt(sq / v.size()) / ref_rms;
            };
            // hopper, automatic token tile
            CK(cudaMemset(dout, 0xFF, (size_t)n * s.rows * 4));
            CK(psionic_clef_hopper_linear(dx, dw, dout, n, s.rows, s.k, hfmt, 0, 0, stream));
            CK(cudaStreamSynchronize(stream));
            CK(cudaMemcpy(got.data(), dout, got.size() * 4, cudaMemcpyDeviceToHost));
            double max_err, rel_rms;
            errors(got, max_err, rel_rms);
            // chunk invariance: 64-row chunks, and each forced token tile
            bool chunk_same = true;
            for (int first = 0; first < n; first += 64) {
                int c = n - first < 64 ? n - first : 64;
                CK(psionic_clef_hopper_linear((const __half *)dx + (size_t)first * s.k, dw,
                                              (float *)dout2 + (size_t)first * s.rows, c, s.rows, s.k, hfmt, 0, 0,
                                              stream));
            }
            CK(cudaStreamSynchronize(stream));
            CK(cudaMemcpy(got2.data(), dout2, got2.size() * 4, cudaMemcpyDeviceToHost));
            chunk_same = memcmp(got.data(), got2.data(), got.size() * 4) == 0;
            std::string tiles_same;
            for (int nt : {64, 128, 192, 256}) {
                CK(psionic_clef_hopper_linear(dx, dw, dout2, n, s.rows, s.k, hfmt, 0, nt, stream));
                CK(cudaStreamSynchronize(stream));
                CK(cudaMemcpy(got2.data(), dout2, got2.size() * 4, cudaMemcpyDeviceToHost));
                bool same = memcmp(got.data(), got2.data(), got.size() * 4) == 0;
                chunk_same = chunk_same && same;
                tiles_same += same ? "=" : "X";
            }
            // accumulate: out = 1 + ref
            std::vector<float> ones((size_t)n * s.rows, 1.0f);
            CK(cudaMemcpy(dout2, ones.data(), ones.size() * 4, cudaMemcpyHostToDevice));
            CK(psionic_clef_hopper_linear(dx, dw, dout2, n, s.rows, s.k, hfmt, 1, 0, stream));
            CK(cudaStreamSynchronize(stream));
            CK(cudaMemcpy(got2.data(), dout2, got2.size() * 4, cudaMemcpyDeviceToHost));
            bool acc_ok = true;
            for (size_t i = 0; i < got2.size(); ++i) {
                if (got2[i] != 1.0f + got[i]) {
                    acc_ok = false;
                    break;
                }
            }
            // the up projection fused with the SiLU product (#11444) against
            // the f32 output followed by silu_mul_to_f16_kernel
            bool silu_ok = true;
            {
                std::vector<float> gate((size_t)n * s.rows);
                for (auto &v : gate) v = normal(rng) * 3.0f;
                void *dgate, *d16b;
                CK(cudaMalloc(&dgate, gate.size() * 4));
                CK(cudaMalloc(&d16b, gate.size() * 2));
                CK(cudaMemcpy(dgate, gate.data(), gate.size() * 4, cudaMemcpyHostToDevice));
                CK(cudaMemcpy(dout2, got.data(), got.size() * 4, cudaMemcpyHostToDevice));
                CK(psionic_clef_silu_mul_to_f16(dgate, dout2, d16b, (long long)gate.size(), stream));
                CK(psionic_clef_hopper_linear_silu(dx, dw, dgate, d16, n, s.rows, s.k, hfmt, 0, stream));
                CK(cudaStreamSynchronize(stream));
                std::vector<uint16_t> a(gate.size()), b(gate.size());
                CK(cudaMemcpy(a.data(), d16, a.size() * 2, cudaMemcpyDeviceToHost));
                CK(cudaMemcpy(b.data(), d16b, b.size() * 2, cudaMemcpyDeviceToHost));
                silu_ok = memcmp(a.data(), b.data(), a.size() * 2) == 0;
                acc_ok = acc_ok && silu_ok;
                cudaFree(dgate);
                cudaFree(d16b);
            }
            const int force_tile = getenv("HOPPER_TILE") ? atoi(getenv("HOPPER_TILE")) : 0;
            float hop = time_ms([&] { psionic_clef_hopper_linear(dx, dw, dout, n, s.rows, s.k, hfmt, 0, force_tile, stream); });
            bool ok = chunk_same && acc_ok && rel_rms < (hfmt == 1 ? 6e-4 : 1e-4);
            all_ok = all_ok && ok;
            printf("%-5s %-32s n=%5d hopper %8.3f ms %6.1f TF %4.1f%%  max_err %.3g rel_rms %.3g  chunks %s tiles %s acc %s silu %s%s\n",
                   s.model, s.name, n, hop, flop / hop / 1e9, 100.0 * flop / hop / 1e9 / peak_tf, max_err, rel_rms,
                   chunk_same ? "bitwise" : "DIFFERS", tiles_same.c_str(), acc_ok ? "ok" : "BAD", silu_ok ? "bitwise" : "DIFFERS",
                   ok ? "" : "  <-- FAIL");
            // today's paths
            float fused = time_ms([&] { psionic_clef_fused_linear(dx, dw, dout, n, s.rows, s.k, s.format, 16, 0, stream); });
            CK(cudaStreamSynchronize(stream));
            CK(cudaMemcpy(got2.data(), dout, got2.size() * 4, cudaMemcpyDeviceToHost));
            double fmax_err, frel;
            errors(got2, fmax_err, frel);
            __half one_h = __float2half(1.0f), zero_h = __float2half(0.0f);
            float deq_cublas = time_ms([&] {
                dequant();
                cublasGemmEx(handle, CUBLAS_OP_T, CUBLAS_OP_N, s.rows, n, s.k, &one_h, dw16, CUDA_R_16F, s.k, dx,
                             CUDA_R_16F, s.k, &zero_h, d16, CUDA_R_16F, s.rows, CUBLAS_COMPUTE_16F,
                             CUBLAS_GEMM_DEFAULT);
                psionic_clef_f16_to_f32(d16, dout, (long long)n * s.rows, 0, stream);
            });
            float res16 = time_ms([&] {
                cublasGemmEx(handle, CUBLAS_OP_T, CUBLAS_OP_N, s.rows, n, s.k, &one_h, dw16, CUDA_R_16F, s.k, dx,
                             CUDA_R_16F, s.k, &zero_h, d16, CUDA_R_16F, s.rows, CUBLAS_COMPUTE_16F,
                             CUBLAS_GEMM_DEFAULT);
            });
            float res32 = time_ms([&] {
                cublasGemmEx(handle, CUBLAS_OP_T, CUBLAS_OP_N, s.rows, n, s.k, &one, dw16, CUDA_R_16F, s.k, dx,
                             CUDA_R_16F, s.k, &zero, dout, CUDA_R_32F, s.rows, CUBLAS_COMPUTE_32F,
                             CUBLAS_GEMM_DEFAULT);
            });
            printf("%-5s %-32s n=%5d   fused %.3f ms %.1f TF (rel_rms %.3g) | dequant+cublas16 %.3f ms %.1f TF | "
                   "resident cublas16 %.3f ms %.1f TF, cublas32 %.3f ms %.1f TF\n",
                   s.model, s.name, n, fused, flop / fused / 1e9, frel, deq_cublas, flop / deq_cublas / 1e9, res16,
                   flop / res16 / 1e9, res32, flop / res32 / 1e9);
            fflush(stdout);
        }
        cudaFree(dw);
        cudaFree(dx);
        cudaFree(dw16);
        cudaFree(dref);
        cudaFree(dout);
        cudaFree(dout2);
        cudaFree(d16);
    }
    printf("%s\n", all_ok ? "ALL OK" : "SOME FAILED");
    return all_ok ? 0 : 1;
}
