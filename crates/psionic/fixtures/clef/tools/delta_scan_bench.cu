// Standalone check of the staged delta scan (psionic_clef_delta_seq with
// staged = 1) against the per-warp scan, at Clef-Flash shapes (16 key heads,
// 32 value heads, dim 128): max output / state difference and timing.
//
//   nvcc -std=c++17 -O3 -arch=sm_89 -I<psionic-backend-cuda>/src/kernels \
//        delta_scan_bench.cu -o delta_scan_bench
//   ./delta_scan_bench [n] [iters]
#include "clef_prefill.cu"

#include <cmath>
#include <cstdio>
#include <cstdlib>
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
    const int n = argc > 1 ? atoi(argv[1]) : 1082;
    const int iters = argc > 2 ? atoi(argv[2]) : 20;
    const int kh = 16, vh = 32, dim = 128, conv_width = 2 * kh * dim + vh * dim, value_offset = 2 * kh * dim;
    std::mt19937 rng(11);
    std::normal_distribution<float> normal(0.0f, 1.0f);
    std::uniform_real_distribution<float> unit(0.0f, 1.0f);
    std::vector<float> qn((size_t)n * kh * dim), kn(qn.size()), conv((size_t)n * conv_width), decay((size_t)n * vh),
        beta(decay.size()), kq((size_t)n * kh);
    for (int t = 0; t < n; ++t)
        for (int h = 0; h < kh; ++h) {
            double nk = 0, nq = 0;
            float *k = &kn[((size_t)t * kh + h) * dim], *q = &qn[((size_t)t * kh + h) * dim];
            for (int i = 0; i < dim; ++i) {
                k[i] = normal(rng);
                q[i] = normal(rng);
                nk += k[i] * k[i];
                nq += q[i] * q[i];
            }
            float dot = 0;
            for (int i = 0; i < dim; ++i) {
                k[i] /= sqrt(nk);
                q[i] /= sqrt(nq) * sqrt((float)dim);
                dot += k[i] * q[i];
            }
            kq[(size_t)t * kh + h] = dot;
        }
    for (auto &v : conv) v = normal(rng);
    for (auto &v : decay) v = 0.9f + 0.1f * unit(rng);
    for (auto &v : beta) v = unit(rng);
    auto up = [](const std::vector<float> &h) {
        void *d;
        CK(cudaMalloc(&d, h.size() * 4));
        CK(cudaMemcpy(d, h.data(), h.size() * 4, cudaMemcpyHostToDevice));
        return d;
    };
    void *dq = up(qn), *dk = up(kn), *dc = up(conv), *dd = up(decay), *db = up(beta), *dkq = up(kq);
    void *state[2], *out[2];
    const size_t state_len = (size_t)vh * dim * dim, out_len = (size_t)n * vh * dim;
    cudaStream_t stream;
    CK(cudaStreamCreate(&stream));
    std::vector<float> got_out[2], got_state[2];
    float ms[2];
    for (int staged = 0; staged < 2; ++staged) {
        CK(cudaMalloc(&state[staged], state_len * 4));
        CK(cudaMalloc(&out[staged], out_len * 4));
        CK(cudaMemset(state[staged], 0, state_len * 4));
        // two calls (a chunk boundary) so the carried state is exercised
        const int first = n / 3;
        CK(psionic_clef_delta_seq(dq, dk, dc, dd, db, dkq, state[staged], out[staged], first, kh, vh, dim, 0,
                                  conv_width, value_offset, staged, stream));
        CK(psionic_clef_delta_seq((float *)dq + (size_t)first * kh * dim, (float *)dk + (size_t)first * kh * dim,
                                  (float *)dc + (size_t)first * conv_width, (float *)dd + (size_t)first * vh,
                                  (float *)db + (size_t)first * vh, (float *)dkq + (size_t)first * kh, state[staged],
                                  (float *)out[staged] + (size_t)first * vh * dim, n - first, kh, vh, dim, 0,
                                  conv_width, value_offset, staged, stream));
        CK(cudaStreamSynchronize(stream));
        got_out[staged].resize(out_len);
        got_state[staged].resize(state_len);
        CK(cudaMemcpy(got_out[staged].data(), out[staged], out_len * 4, cudaMemcpyDeviceToHost));
        CK(cudaMemcpy(got_state[staged].data(), state[staged], state_len * 4, cudaMemcpyDeviceToHost));
        cudaEvent_t e0, e1;
        cudaEventCreate(&e0);
        cudaEventCreate(&e1);
        cudaEventRecord(e0, stream);
        for (int i = 0; i < iters; ++i)
            CK(psionic_clef_delta_seq(dq, dk, dc, dd, db, dkq, state[staged], out[staged], n, kh, vh, dim, 0,
                                      conv_width, value_offset, staged, stream));
        cudaEventRecord(e1, stream);
        cudaEventSynchronize(e1);
        cudaEventElapsedTime(&ms[staged], e0, e1);
        ms[staged] /= iters;
    }
    double out_err = 0, out_scale = 0, state_err = 0, state_scale = 0;
    for (size_t i = 0; i < out_len; ++i) {
        out_err = fmax(out_err, fabs(got_out[0][i] - got_out[1][i]));
        out_scale = fmax(out_scale, fabs(got_out[0][i]));
    }
    for (size_t i = 0; i < state_len; ++i) {
        state_err = fmax(state_err, fabs(got_state[0][i] - got_state[1][i]));
        state_scale = fmax(state_scale, fabs(got_state[0][i]));
    }
    printf("n=%d per-warp %.3f ms, staged %.3f ms (%.1fx); max |dout| %.3g of %.3g, max |dstate| %.3g of %.3g\n", n,
           ms[0], ms[1], ms[0] / ms[1], out_err, out_scale, state_err, state_scale);
    return 0;
}
