// Standalone check of psionic_clef_fused_linear (clef_prefill.cu) against
// dequantize + cuBLAS, on random Q8_0 / Q4_K weights at Clef-Flash shapes:
// max error vs an f32-accumulate reference, bitwise chunk invariance, and
// timing. Build on the CUDA box:
//
//   nvcc -std=c++17 -O3 -arch=sm_89 -I<psionic-backend-cuda>/src/kernels \
//        fused_linear_bench.cu -lcublas -o fused_linear_bench
//   ./fused_linear_bench [iters] [n ...]
#include "clef_prefill.cu"

#include <cublas_v2.h>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <random>
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

int main(int argc, char **argv) {
    int iters = argc > 1 ? atoi(argv[1]) : 20;
    struct Shape {
        const char *name;
        int format, rows, k;
    } shapes[] = {
        {"ffn_gate/up Q4_K 12288x4096", 1, 12288, 4096}, {"ffn_down Q4_K 4096x12288", 1, 4096, 12288},
        {"attn_qkv Q8_0 8192x4096", 0, 8192, 4096},      {"ssm_out Q8_0 4096x4096", 0, 4096, 4096},
        {"attn_k Q8_0 1024x4096", 0, 1024, 4096},        {"alpha Q8_0 32x4096", 0, 32, 4096},
    };
    std::vector<int> ns;
    for (int i = 2; i < argc; ++i) ns.push_back(atoi(argv[i]));
    if (ns.empty()) ns = {1082, 2048, 3917};
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
    for (auto &s : shapes) {
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
        if (s.format == 0)
            CK(psionic_clef_dequant_q8_0_f16(dw, dw16, elements / 32, stream));
        else
            CK(psionic_clef_dequant_q4_k_f16(dw, dw16, elements / 256, stream));
        for (int n : ns) {
            float one = 1.0f, zero = 0.0f;
            // reference: f16 weights, f32 accumulate (column-major: out^T[rows, n] = W . x^T)
            CK(cublasGemmEx(handle, CUBLAS_OP_T, CUBLAS_OP_N, s.rows, n, s.k, &one, dw16, CUDA_R_16F, s.k, dx,
                            CUDA_R_16F, s.k, &zero, dref, CUDA_R_32F, s.rows, CUBLAS_COMPUTE_32F,
                            CUBLAS_GEMM_DEFAULT));
            std::vector<float> ref((size_t)n * s.rows), got((size_t)n * s.rows), got2((size_t)n * s.rows);
            CK(cudaMemcpy(ref.data(), dref, ref.size() * 4, cudaMemcpyDeviceToHost));
            double ref_rms = 0;
            for (float v : ref) ref_rms += (double)v * v;
            ref_rms = sqrt(ref_rms / ref.size());
            for (int seg : {16}) {
                CK(psionic_clef_fused_linear(dx, dw, dout, n, s.rows, s.k, s.format, seg, 0, stream));
                CK(cudaStreamSynchronize(stream));
                CK(cudaMemcpy(got.data(), dout, got.size() * 4, cudaMemcpyDeviceToHost));
                double max_err = 0, sq = 0;
                for (size_t i = 0; i < got.size(); ++i) {
                    double e = fabs((double)got[i] - ref[i]);
                    max_err = fmax(max_err, e);
                    sq += e * e;
                }
                // chunk invariance: the same rows in chunks of 64 (offset x / out)
                for (int first = 0; first < n; first += 64) {
                    int c = n - first < 64 ? n - first : 64;
                    CK(psionic_clef_fused_linear((const __half *)dx + (size_t)first * s.k, dw,
                                                 (float *)dout2 + (size_t)first * s.rows, c, s.rows, s.k, s.format,
                                                 seg, 0, stream));
                }
                CK(cudaStreamSynchronize(stream));
                CK(cudaMemcpy(got2.data(), dout2, got2.size() * 4, cudaMemcpyDeviceToHost));
                bool same = memcmp(got.data(), got2.data(), got.size() * 4) == 0;
                cudaEventRecord(e0, stream);
                for (int it = 0; it < iters; ++it)
                    CK(psionic_clef_fused_linear(dx, dw, dout, n, s.rows, s.k, s.format, seg, 0, stream));
                cudaEventRecord(e1, stream);
                cudaEventSynchronize(e1);
                float ms;
                cudaEventElapsedTime(&ms, e0, e1);
                ms /= iters;
                printf("%-30s n=%5d fused seg=%d: %7.3f ms %6.1f TF  max_err %.3g rel_rms %.3g  chunk64 %s\n", s.name,
                       n, seg, ms, 2.0 * n * s.rows * s.k / ms / 1e9, max_err,
                       sqrt(sq / got.size()) / ref_rms, same ? "bitwise" : "DIFFERS");
            }
            // baselines: dequant + cuBLAS f16 accumulate (+ convert), and f32 accumulate
            __half one_h = __float2half(1.0f), zero_h = __float2half(0.0f);
            for (int mode = 0; mode < 2; ++mode) {
                cudaEventRecord(e0, stream);
                for (int it = 0; it < iters; ++it) {
                    if (s.format == 0)
                        psionic_clef_dequant_q8_0_f16(dw, dw16, elements / 32, stream);
                    else
                        psionic_clef_dequant_q4_k_f16(dw, dw16, elements / 256, stream);
                    if (mode == 0) {
                        CK(cublasGemmEx(handle, CUBLAS_OP_T, CUBLAS_OP_N, s.rows, n, s.k, &one_h, dw16, CUDA_R_16F,
                                        s.k, dx, CUDA_R_16F, s.k, &zero_h, d16, CUDA_R_16F, s.rows,
                                        CUBLAS_COMPUTE_16F, CUBLAS_GEMM_DEFAULT));
                        psionic_clef_f16_to_f32(d16, dout, (long long)n * s.rows, 0, stream);
                    } else {
                        CK(cublasGemmEx(handle, CUBLAS_OP_T, CUBLAS_OP_N, s.rows, n, s.k, &one, dw16, CUDA_R_16F,
                                        s.k, dx, CUDA_R_16F, s.k, &zero, dout, CUDA_R_32F, s.rows,
                                        CUBLAS_COMPUTE_32F, CUBLAS_GEMM_DEFAULT));
                    }
                }
                cudaEventRecord(e1, stream);
                cudaEventSynchronize(e1);
                float ms;
                cudaEventElapsedTime(&ms, e0, e1);
                ms /= iters;
                std::vector<float> base((size_t)n * s.rows);
                CK(cudaMemcpy(base.data(), dout, base.size() * 4, cudaMemcpyDeviceToHost));
                double max_err = 0;
                for (size_t i = 0; i < base.size(); ++i) max_err = fmax(max_err, fabs((double)base[i] - ref[i]));
                printf("%-30s n=%5d dequant+cublas %s: %7.3f ms %6.1f TF  max_err %.3g\n", s.name, n,
                       mode == 0 ? "f16acc" : "f32acc", ms, 2.0 * n * s.rows * s.k / ms / 1e9, max_err);
            }
        }
        cudaFree(dw);
        cudaFree(dx);
        cudaFree(dw16);
        cudaFree(dref);
        cudaFree(dout);
        cudaFree(dout2);
        cudaFree(d16);
    }
    return 0;
}
