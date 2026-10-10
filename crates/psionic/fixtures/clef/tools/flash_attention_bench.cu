// Standalone check of psionic_clef_flash_attention (clef_prefill.cu): a
// double-precision reference on sampled rows, bitwise chunk invariance
// (whole vs chunks of 2048, 512 and 64 at their positions), and timing.
// Clef-Flash shapes: 16 heads, 4 KV heads, head dim 256.
//
//   nvcc -std=c++17 -O3 -arch=sm_89 -I<psionic-backend-cuda>/src/kernels \
//        flash_attention_bench.cu -o flash_attention_bench
//   ./flash_attention_bench [length] [iters]
#include "clef_prefill.cu"

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

int main(int argc, char **argv) {
    const int L = argc > 1 ? atoi(argv[1]) : 4096;
    const int iters = argc > 2 ? atoi(argv[2]) : 5;
    const int heads = 16, kvh = 4, dim = 256;
    std::mt19937 rng(5);
    std::normal_distribution<float> normal(0.0f, 1.0f);
    const float scale = 1.0f / 16.0f;
    std::vector<__half> q((size_t)L * heads * dim), k((size_t)L * kvh * dim), v(k.size());
    for (auto &x : q) x = __float2half(normal(rng) * scale * 2.0f);
    for (auto &x : k) x = __float2half(normal(rng) * 2.0f);
    for (auto &x : v) x = __float2half(normal(rng));
    void *dq, *dk, *dv, *dout, *dout2;
    CK(cudaMalloc(&dq, q.size() * 2));
    CK(cudaMalloc(&dk, k.size() * 2));
    CK(cudaMalloc(&dv, v.size() * 2));
    CK(cudaMalloc(&dout, (size_t)L * heads * dim * 4));
    CK(cudaMalloc(&dout2, (size_t)L * heads * dim * 4));
    CK(cudaMemcpy(dq, q.data(), q.size() * 2, cudaMemcpyHostToDevice));
    CK(cudaMemcpy(dk, k.data(), k.size() * 2, cudaMemcpyHostToDevice));
    CK(cudaMemcpy(dv, v.data(), v.size() * 2, cudaMemcpyHostToDevice));
    cudaStream_t stream;
    CK(cudaStreamCreate(&stream));
    CK(psionic_clef_flash_attention(dq, dk, dv, dout, L, heads, kvh, dim, 0, stream));
    CK(cudaStreamSynchronize(stream));
    std::vector<float> whole((size_t)L * heads * dim), part(whole.size());
    CK(cudaMemcpy(whole.data(), dout, whole.size() * 4, cudaMemcpyDeviceToHost));
    // reference on sampled rows
    double worst = 0, scale_ref = 0;
    for (int t : {0, 1, 31, 32, 63, 100, L / 2, L - 1}) {
        for (int h : {0, 5, 15}) {
            const int kh = h / (heads / kvh);
            std::vector<double> s(t + 1);
            double mx = -1e300;
            for (int j = 0; j <= t; ++j) {
                double d = 0;
                for (int i = 0; i < dim; ++i)
                    d += (double)__half2float(q[((size_t)t * heads + h) * dim + i]) *
                         (double)__half2float(k[((size_t)j * kvh + kh) * dim + i]);
                s[j] = d;
                mx = fmax(mx, d);
            }
            double sum = 0;
            for (auto &x : s) sum += (x = exp(x - mx));
            for (int i = 0; i < dim; ++i) {
                double o = 0;
                for (int j = 0; j <= t; ++j) o += s[j] / sum * (double)__half2float(v[((size_t)j * kvh + kh) * dim + i]);
                worst = fmax(worst, fabs(o - whole[((size_t)t * heads + h) * dim + i]));
                scale_ref = fmax(scale_ref, fabs(o));
            }
        }
    }
    printf("L=%d: max |out - f64 reference| %.3g (outputs up to %.3g)\n", L, worst, scale_ref);
    for (int chunk : {2048, 512, 64}) {
        for (int first = 0; first < L; first += chunk) {
            const int n = L - first < chunk ? L - first : chunk;
            CK(psionic_clef_flash_attention((const __half *)dq + (size_t)first * heads * dim, dk, dv,
                                            (float *)dout2 + (size_t)first * heads * dim, n, heads, kvh, dim, first,
                                            stream));
        }
        CK(cudaStreamSynchronize(stream));
        CK(cudaMemcpy(part.data(), dout2, part.size() * 4, cudaMemcpyDeviceToHost));
        printf("chunk %d vs whole: %s\n", chunk, memcmp(part.data(), whole.data(), part.size() * 4) == 0 ? "bitwise" : "DIFFERS");
    }
    cudaEvent_t e0, e1;
    cudaEventCreate(&e0);
    cudaEventCreate(&e1);
    cudaEventRecord(e0, stream);
    for (int i = 0; i < iters; ++i) CK(psionic_clef_flash_attention(dq, dk, dv, dout, L, heads, kvh, dim, 0, stream));
    cudaEventRecord(e1, stream);
    cudaEventSynchronize(e1);
    float ms;
    cudaEventElapsedTime(&ms, e0, e1);
    ms /= iters;
    const double flops = 4.0 * heads * dim * (double)L * (L + 1) / 2;
    printf("L=%d one layer: %.3f ms, %.1f TF (causal)\n", L, ms, flops / ms / 1e9);
    return 0;
}
