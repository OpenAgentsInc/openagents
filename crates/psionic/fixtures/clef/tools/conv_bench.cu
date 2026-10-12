// Standalone check of the register-window causal conv (psionic_clef_conv1d_seq_silu
// with k = 4, #11444) against the per-element kernel: bitwise output and
// carried state, and timing. Channels 8,192 (Clef-Flash) or 10,240 (27B).
//
//   nvcc -std=c++17 -O3 -arch=sm_90 -I<psionic-backend-cuda>/src/kernels conv_bench.cu -o conv_bench
//   ./conv_bench [n] [channels] [iters]
#include "clef_prefill.cu"

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
    const int n = argc > 1 ? atoi(argv[1]) : 2048;
    const int ch = argc > 2 ? atoi(argv[2]) : 8192;
    const int iters = argc > 3 ? atoi(argv[3]) : 20;
    const int k = 4;
    std::mt19937 rng(3);
    std::normal_distribution<float> normal(0.0f, 1.0f);
    std::vector<float> in((size_t)n * ch), state((size_t)ch * (k - 1)), w((size_t)ch * k);
    for (auto &v : in) v = normal(rng);
    for (auto &v : state) v = normal(rng);
    for (auto &v : w) v = normal(rng) * 0.5f;
    float *din, *dstate, *dstate2, *dw, *dout, *dout2;
    CK(cudaMalloc(&din, in.size() * 4));
    CK(cudaMalloc(&dstate, state.size() * 4));
    CK(cudaMalloc(&dstate2, state.size() * 4));
    CK(cudaMalloc(&dw, w.size() * 4));
    CK(cudaMalloc(&dout, in.size() * 4));
    CK(cudaMalloc(&dout2, in.size() * 4));
    CK(cudaMemcpy(din, in.data(), in.size() * 4, cudaMemcpyHostToDevice));
    CK(cudaMemcpy(dstate, state.data(), state.size() * 4, cudaMemcpyHostToDevice));
    CK(cudaMemcpy(dstate2, state.data(), state.size() * 4, cudaMemcpyHostToDevice));
    CK(cudaMemcpy(dw, w.data(), w.size() * 4, cudaMemcpyHostToDevice));
    cudaStream_t stream;
    CK(cudaStreamCreate(&stream));
    // reference: the per-element kernel and the state kernel
    conv1d_seq_silu_kernel<<<blocks_for((long long)n * ch, 256), 256, 0, stream>>>(din, dstate2, dw, dout2, n, ch, k);
    conv1d_state_kernel<<<blocks_for(ch, 256), 256, 0, stream>>>(din, dstate2, n, ch, k);
    CK(psionic_clef_conv1d_seq_silu(din, dstate, dw, dout, n, ch, k, stream));
    CK(cudaStreamSynchronize(stream));
    std::vector<float> a(in.size()), b(in.size()), sa(state.size()), sb(state.size());
    CK(cudaMemcpy(a.data(), dout, a.size() * 4, cudaMemcpyDeviceToHost));
    CK(cudaMemcpy(b.data(), dout2, b.size() * 4, cudaMemcpyDeviceToHost));
    CK(cudaMemcpy(sa.data(), dstate, sa.size() * 4, cudaMemcpyDeviceToHost));
    CK(cudaMemcpy(sb.data(), dstate2, sb.size() * 4, cudaMemcpyDeviceToHost));
    const bool same = memcmp(a.data(), b.data(), a.size() * 4) == 0 && memcmp(sa.data(), sb.data(), sa.size() * 4) == 0;
    cudaEvent_t e0, e1;
    cudaEventCreate(&e0);
    cudaEventCreate(&e1);
    auto time_ms = [&](bool fast) {
        cudaEventRecord(e0, stream);
        for (int i = 0; i < iters; ++i) {
            if (fast) {
                psionic_clef_conv1d_seq_silu(din, dstate, dw, dout, n, ch, k, stream);
            } else {
                conv1d_seq_silu_kernel<<<blocks_for((long long)n * ch, 256), 256, 0, stream>>>(din, dstate2, dw, dout2,
                                                                                                n, ch, k);
                conv1d_state_kernel<<<blocks_for(ch, 256), 256, 0, stream>>>(din, dstate2, n, ch, k);
            }
        }
        cudaEventRecord(e1, stream);
        cudaEventSynchronize(e1);
        float ms;
        cudaEventElapsedTime(&ms, e0, e1);
        return ms / iters;
    };
    const float old_ms = time_ms(false), new_ms = time_ms(true);
    printf("n=%d channels=%d: per-element %.3f ms, register window %.3f ms (%.2fx); %s\n", n, ch, old_ms, new_ms,
           old_ms / new_ms, same ? "bitwise" : "DIFFERS");
    return same ? 0 : 1;
}
