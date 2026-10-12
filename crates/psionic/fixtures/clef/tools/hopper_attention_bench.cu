// Standalone check of psionic_clef_hopper_attention (clef_hopper.cu, #11444)
// against a double-precision reference on sampled rows, bitwise chunk
// invariance (whole vs chunks of 2048, 512 and 64 at their positions), the
// fused sigmoid gate against the separate kernel, and timing against
// psionic_clef_flash_attention (clef_prefill.cu) where that takes the shape.
// Shapes: Clef-Flash 16 heads / 4 KV heads, Clef 27B 24 / 4; head dim 256.
//
//   nvcc -std=c++17 -O3 -gencode=arch=compute_90a,code=sm_90a \
//        -I<psionic-backend-cuda>/src/kernels hopper_attention_bench.cu -lcuda -o hopper_attention_bench
//   ./hopper_attention_bench [length] [heads] [iters]
#include "clef_prefill.cu"
#include "clef_hopper.cu"

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
    const int heads = argc > 2 ? atoi(argv[2]) : 16;
    const int iters = argc > 3 ? atoi(argv[3]) : 5;
    const int kvh = 4, dim = 256, group = heads / kvh;
    std::mt19937 rng(5);
    std::normal_distribution<float> normal(0.0f, 1.0f);
    const float scale = 1.0f / 16.0f;
    std::vector<__half> q((size_t)L * heads * dim), k((size_t)L * kvh * dim), v(k.size());
    std::vector<float> gate((size_t)L * heads * dim);
    for (auto &x : q) x = __float2half(normal(rng) * scale * 2.0f);
    for (auto &x : k) x = __float2half(normal(rng) * 2.0f);
    for (auto &x : v) x = __float2half(normal(rng));
    for (auto &x : gate) x = normal(rng);
    void *dq, *dk, *dv, *dout, *dout2, *dgate, *d16a, *d16b;
    const size_t out_len = (size_t)L * heads * dim;
    CK(cudaMalloc(&dq, q.size() * 2));
    CK(cudaMalloc(&dk, k.size() * 2));
    CK(cudaMalloc(&dv, v.size() * 2));
    CK(cudaMalloc(&dout, out_len * 4));
    CK(cudaMalloc(&dout2, out_len * 4));
    CK(cudaMalloc(&dgate, out_len * 4));
    CK(cudaMalloc(&d16a, out_len * 2));
    CK(cudaMalloc(&d16b, out_len * 2));
    CK(cudaMemcpy(dq, q.data(), q.size() * 2, cudaMemcpyHostToDevice));
    CK(cudaMemcpy(dk, k.data(), k.size() * 2, cudaMemcpyHostToDevice));
    CK(cudaMemcpy(dv, v.data(), v.size() * 2, cudaMemcpyHostToDevice));
    CK(cudaMemcpy(dgate, gate.data(), out_len * 4, cudaMemcpyHostToDevice));
    cudaStream_t stream;
    CK(cudaStreamCreate(&stream));
    CK(psionic_clef_hopper_attention(dq, dk, dv, dout, nullptr, nullptr, L, heads, kvh, dim, 0, stream));
    CK(cudaStreamSynchronize(stream));
    std::vector<float> whole(out_len), part(out_len);
    CK(cudaMemcpy(whole.data(), dout, out_len * 4, cudaMemcpyDeviceToHost));
    double worst = 0, scale_ref = 0;
    for (int t : {0, 1, 31, 63, 64, 65, 100, 127, 128, L / 2, L - 65, L - 1}) {
        if (t < 0 || t >= L) continue;
        for (int h : {0, 1, 5, heads - 1}) {
            const int kh = h / group;
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
    printf("L=%d heads=%d: max |out - f64 reference| %.3g (outputs up to %.3g)\n", L, heads, worst, scale_ref);
    bool ok = worst < 4e-4;
    for (int chunk : {2048, 512, 64}) {
        for (int first = 0; first < L; first += chunk) {
            const int n = L - first < chunk ? L - first : chunk;
            CK(psionic_clef_hopper_attention((const __half *)dq + (size_t)first * heads * dim, dk, dv,
                                             (float *)dout2 + (size_t)first * heads * dim, nullptr, nullptr, n, heads,
                                             kvh, dim, first, stream));
        }
        CK(cudaStreamSynchronize(stream));
        CK(cudaMemcpy(part.data(), dout2, out_len * 4, cudaMemcpyDeviceToHost));
        const bool same = memcmp(part.data(), whole.data(), out_len * 4) == 0;
        ok = ok && same;
        printf("chunk %d vs whole: %s\n", chunk, same ? "bitwise" : "DIFFERS");
    }
    // fused gate == separate sigmoid kernel on the f32 output
    CK(psionic_clef_hopper_attention(dq, dk, dv, nullptr, dgate, d16a, L, heads, kvh, dim, 0, stream));
    CK(psionic_clef_sigmoid_gate_to_f16(dout, dgate, d16b, (long long)out_len, stream));
    CK(cudaStreamSynchronize(stream));
    {
        std::vector<uint16_t> a(out_len), b(out_len);
        CK(cudaMemcpy(a.data(), d16a, out_len * 2, cudaMemcpyDeviceToHost));
        CK(cudaMemcpy(b.data(), d16b, out_len * 2, cudaMemcpyDeviceToHost));
        const bool same = memcmp(a.data(), b.data(), out_len * 2) == 0;
        ok = ok && same;
        printf("fused gate vs separate: %s\n", same ? "bitwise" : "DIFFERS");
    }
    cudaEvent_t e0, e1;
    cudaEventCreate(&e0);
    cudaEventCreate(&e1);
    auto time_chunks = [&](bool hopper, int chunk) {
        cudaEventRecord(e0, stream);
        for (int i = 0; i < iters; ++i) {
            for (int first = 0; first < L; first += chunk) {
                const int n = L - first < chunk ? L - first : chunk;
                const __half *qq = (const __half *)dq + (size_t)first * heads * dim;
                float *oo = (float *)dout2 + (size_t)first * heads * dim;
                if (hopper) {
                    CK(psionic_clef_hopper_attention(qq, dk, dv, oo, nullptr, nullptr, n, heads, kvh, dim, first, stream));
                } else {
                    CK(psionic_clef_flash_attention(qq, dk, dv, oo, n, heads, kvh, dim, first, stream));
                }
            }
        }
        cudaEventRecord(e1, stream);
        cudaEventSynchronize(e1);
        float ms;
        cudaEventElapsedTime(&ms, e0, e1);
        return ms / iters;
    };
    const double flops = 4.0 * heads * dim * (double)L * (L + 1) / 2;
    for (int chunk : {L, 2048}) {
        const float h = time_chunks(true, chunk);
        printf("L=%d chunk %d, one layer: hopper %.3f ms (%.1f TF)", L, chunk, h, flops / h / 1e9);
        if (group == 4) {
            const float f = time_chunks(false, chunk);
            printf(", flash256 %.3f ms (%.1f TF), %.2fx", f, flops / f / 1e9, f / h);
        }
        printf("\n");
    }
    printf("%s\n", ok ? "PASS" : "FAIL");
    return ok ? 0 : 1;
}
