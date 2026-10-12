// Standalone check of the blocked tensor-core delta scan
// (psionic_clef_delta_block, clef_prefill.cu, #11444) against the per-warp
// scan and the staged scan:
// max output / state difference, bitwise chunk invariance (whole vs chunks
// of 2048, 512 and 64 with the state carried), and timing per 2,048-token
// chunk. Shapes: 16 key heads, dim 128, value heads 32 (Clef-Flash) or 48
// (Clef 27B).
//
//   nvcc -std=c++17 -O3 -arch=sm_90 -I<psionic-backend-cuda>/src/kernels \
//        delta_block_bench.cu -o delta_block_bench
//   ./delta_block_bench [n] [value_heads] [iters]
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
    const int n = argc > 1 ? atoi(argv[1]) : 4100;
    const int vh = argc > 2 ? atoi(argv[2]) : 32;
    const int iters = argc > 3 ? atoi(argv[3]) : 10;
    const int kh = 16, dim = 128, conv_width = 2 * kh * dim + vh * dim, value_offset = 2 * kh * dim;
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
    // decays from about 0.02 to 1 (real layers span that), betas in (0, 1)
    for (auto &v : decay) v = expf(-4.0f * unit(rng) * unit(rng) * unit(rng));
    for (auto &v : beta) v = unit(rng);
    auto up = [](const std::vector<float> &h) {
        void *d;
        CK(cudaMalloc(&d, h.size() * 4));
        CK(cudaMemcpy(d, h.data(), h.size() * 4, cudaMemcpyHostToDevice));
        return d;
    };
    float *dq = (float *)up(qn), *dk = (float *)up(kn), *dc = (float *)up(conv), *dd = (float *)up(decay),
          *db = (float *)up(beta), *dkq = (float *)up(kq);
    const size_t state_len = (size_t)vh * dim * dim, out_len = (size_t)n * vh * dim;
    void *coef, *state, *out;
    CK(cudaMalloc(&coef, (size_t)((n + 15) / 16) * vh * 544 * 4));
    CK(cudaMalloc(&state, state_len * 4));
    CK(cudaMalloc(&out, out_len * 4));
    cudaStream_t stream;
    CK(cudaStreamCreate(&stream));
    // mode: 0 per-warp, 1 staged, 2/3/4 blocked on tensor cores with 1/2/4 warps per 16 rows
    const int variants[5] = {0, 0, 101, 102, 104};
    auto run = [&](int mode, int chunk) {
        for (int first = 0; first < n; first += chunk) {
            const int m = n - first < chunk ? n - first : chunk;
            float *q = dq + (size_t)first * kh * dim, *k = dk + (size_t)first * kh * dim,
                  *c = dc + (size_t)first * conv_width, *g = dd + (size_t)first * vh, *b = db + (size_t)first * vh,
                  *o = (float *)out + (size_t)first * vh * dim;
            if (mode < 2) {
                CK(psionic_clef_delta_seq(q, k, c, g, b, dkq + (size_t)first * kh, state, o, m, kh, vh, dim, 0,
                                          conv_width, value_offset, mode, stream));
            } else {
                CK(psionic_clef_delta_block(q, k, c, g, b, coef, state, o, m, kh, vh, dim, 0, conv_width, value_offset,
                                            variants[mode], stream));
            }
        }
    };
    auto result = [&](int mode, int chunk, std::vector<float> &o, std::vector<float> &st) {
        CK(cudaMemset(state, 0, state_len * 4));
        run(mode, chunk);
        CK(cudaStreamSynchronize(stream));
        o.resize(out_len);
        st.resize(state_len);
        CK(cudaMemcpy(o.data(), out, out_len * 4, cudaMemcpyDeviceToHost));
        CK(cudaMemcpy(st.data(), state, state_len * 4, cudaMemcpyDeviceToHost));
    };
    std::vector<float> ref_o, ref_s;
    result(0, n, ref_o, ref_s);
    auto diff = [](const std::vector<float> &a, const std::vector<float> &b, double &scale) {
        double worst = 0;
        scale = 0;
        for (size_t i = 0; i < a.size(); ++i) {
            worst = fmax(worst, fabs(a[i] - b[i]));
            scale = fmax(scale, fabs(a[i]));
        }
        return worst;
    };
    bool ok = true;
    const char *names[5] = {"per-warp", "staged", "block x1", "block x2", "block x4"};
    cudaEvent_t e0, e1;
    cudaEventCreate(&e0);
    cudaEventCreate(&e1);
    for (int mode = 0; mode < 5; ++mode) {
        if (getenv("ONLY") != nullptr && atoi(getenv("ONLY")) != mode) {
            continue;
        }
        std::vector<float> o, st;
        result(mode, n, o, st);
        double so, ss;
        const double eo = diff(ref_o, o, so), es = diff(ref_s, st, ss);
        printf("n=%d vh=%d %-9s vs per-warp: max |dout| %.3g of %.3g, max |dstate| %.3g of %.3g", n, vh, names[mode],
               eo, so, es, ss);
        if (mode >= 1) {
            for (int chunk : {2048, 512, 64}) {
                std::vector<float> o2, st2;
                result(mode, chunk, o2, st2);
                const bool same = memcmp(o2.data(), o.data(), out_len * 4) == 0 &&
                                  memcmp(st2.data(), st.data(), state_len * 4) == 0;
                printf(", chunk %d %s", chunk, same ? "bitwise" : "DIFFERS");
                ok = ok && same;
            }
        }
        if (mode >= 2) {
            ok = ok && eo < 1e-5 * fmax(1.0, so) && es < 1e-5 * fmax(1.0, ss);
        }
        // time: chunks of 2048 over n
        CK(cudaMemset(state, 0, state_len * 4));
        cudaEventRecord(e0, stream);
        for (int i = 0; i < iters; ++i) run(mode, 2048);
        cudaEventRecord(e1, stream);
        cudaEventSynchronize(e1);
        float ms;
        cudaEventElapsedTime(&ms, e0, e1);
        printf("; %.3f ms per %d tokens (%.2f us/token)\n", ms / iters, n, 1000.0 * ms / iters / n);
    }
    printf("%s\n", ok ? "PASS" : "FAIL");
    return ok ? 0 : 1;
}
