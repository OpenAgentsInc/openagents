#include "clef_prefill.cu"
#include <algorithm>
#include <cmath>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <vector>

static void checked(cudaError_t code) {
    if (code != cudaSuccess) throw std::runtime_error(cudaGetErrorString(code));
}
static constexpr size_t guard_size = 32;
static constexpr float sentinel = 913.25f;
static size_t live_bytes = 0, peak_bytes = 0;
struct Guarded {
    float *allocation = nullptr;
    std::vector<float> original;
    size_t count;
    explicit Guarded(const std::vector<float>& values) : original(values), count(values.size()) {
        std::vector<float> all(count + 2 * guard_size, sentinel);
        std::copy(values.begin(), values.end(), all.begin() + guard_size);
        const size_t bytes = all.size() * sizeof(float);
        if (live_bytes + bytes > (4u << 20)) throw std::runtime_error("synthetic buffer cap exceeded");
        checked(cudaMalloc(&allocation, bytes));
        live_bytes += bytes;
        peak_bytes = std::max(peak_bytes, live_bytes);
        checked(cudaMemcpy(allocation, all.data(), bytes, cudaMemcpyHostToDevice));
    }
    Guarded(const Guarded&) = delete;
    ~Guarded() { cudaFree(allocation); live_bytes -= (count + 2 * guard_size) * sizeof(float); }
    float* data() { return allocation + guard_size; }
    std::vector<float> read(bool input = false) {
        std::vector<float> all(count + 2 * guard_size);
        checked(cudaMemcpy(all.data(), allocation, all.size() * sizeof(float), cudaMemcpyDeviceToHost));
        for (size_t i = 0; i < guard_size; ++i) {
            if (memcmp(&all[i], &sentinel, sizeof(float)) ||
                memcmp(&all[guard_size + count + i], &sentinel, sizeof(float)))
                throw std::runtime_error("buffer guard changed");
        }
        std::vector<float> values(all.begin() + guard_size, all.begin() + guard_size + count);
        if (input && memcmp(values.data(), original.data(), count * sizeof(float)))
            throw std::runtime_error("input payload changed");
        return values;
    }
};

static void compare(float actual, double expected, double& maximum, size_t& compared) {
    const double error = std::abs(double(actual) - expected);
    if (!std::isfinite(actual) || error > 2e-7 + 3e-6 * std::abs(expected)) {
        std::cerr << "actual=" << actual << " expected=" << expected << " error=" << error << '\n';
        throw std::runtime_error("independent f64 oracle mismatch");
    }
    maximum = std::max(maximum, error);
    ++compared;
}

static size_t exercise(int n, int vh, int dim) {
    const int kh = 16, width = (2 * kh + vh) * dim;
    const float scales[] = {0.0f, 1.0f / 65536, 1.0f / 1024, 0.5f};
    std::vector<float> cv(n * width, 0), av(n * vh), bv(n * vh), aa(vh), dt(vh);
    for (int h = 0; h < vh; ++h) {
        aa[h] = -float(h % 5 + 1) / 4;
        dt[h] = float(h % 3 - 1) / 8;
    }
    for (int t = 0; t < n; ++t) {
        for (int h = 0; h < kh; ++h) {
            for (int i = 0; i < dim; ++i) {
                cv[t * width + h * dim + i] = float((i + h) % 7 - 3) * scales[t % 4];
                cv[t * width + (kh + h) * dim + i] = float((3 * i + h) % 11 - 5) * scales[t % 4];
            }
        }
        for (int h = 0; h < vh; ++h) {
            av[t * vh + h] = float((t + h) % 9 - 4) / 2;
            bv[t * vh + h] = float((3 * t + h) % 11 - 5) / 4;
        }
    }
    Guarded conv(cv), alpha(av), beta_in(bv), ssm_a(aa), ssm_dt(dt);
    Guarded q(std::vector<float>(n * kh * dim, sentinel));
    Guarded k(std::vector<float>(n * kh * dim, sentinel));
    Guarded decay(std::vector<float>(n * vh, sentinel));
    Guarded beta(std::vector<float>(n * vh, sentinel));
    Guarded dots(std::vector<float>(n * kh, sentinel));
    auto launch = [&](int count) {
        checked(static_cast<cudaError_t>(psionic_clef_delta_prep(
            conv.data(), alpha.data(), beta_in.data(), ssm_a.data(), ssm_dt.data(),
            q.data(), k.data(), decay.data(), beta.data(), dots.data(),
            count, kh, vh, dim, width, nullptr)));
        checked(cudaDeviceSynchronize());
    };
    launch(n);
    const auto qv = q.read(), kv = k.read(), dv = decay.read(), betav = beta.read(), dotv = dots.read();
    double q_error = 0, k_error = 0, dot_error = 0, gate_error = 0;
    size_t compared = 0;
    for (int t = 0; t < n; ++t) {
        for (int h = 0; h < kh; ++h) {
            double qq = 1e-6, kk = 1e-6;
            for (int i = 0; i < dim; ++i) {
                const double x = cv[t * width + h * dim + i], y = cv[t * width + (kh + h) * dim + i];
                qq += x * x; kk += y * y;
            }
            double dot = 0;
            for (int i = 0; i < dim; ++i) {
                const double expected_q = double(cv[t * width + h * dim + i]) / std::sqrt(qq * dim);
                const double expected_k = double(cv[t * width + (kh + h) * dim + i]) / std::sqrt(kk);
                compare(qv[(t * kh + h) * dim + i], expected_q, q_error, compared);
                compare(kv[(t * kh + h) * dim + i], expected_k, k_error, compared);
                dot += expected_q * expected_k;
            }
            compare(dotv[t * kh + h], dot, dot_error, compared);
        }
        for (int h = 0; h < vh; ++h) {
            const double x = double(av[t * vh + h]) + double(dt[h]);
            compare(dv[t * vh + h], std::exp(std::log1p(std::exp(x)) * double(aa[h])), gate_error, compared);
            compare(betav[t * vh + h], 1 / (1 + std::exp(-double(bv[t * vh + h]))), gate_error, compared);
        }
    }
    auto identical = [](const std::vector<float>& left, const std::vector<float>& right) {
        if (left.size() != right.size() || memcmp(left.data(), right.data(), left.size() * sizeof(float)))
            throw std::runtime_error("repeat or zero-length launch changed output");
    };
    for (int count : {n, 0}) {
        launch(count);
        identical(qv, q.read()); identical(kv, k.read()); identical(dv, decay.read());
        identical(betav, beta.read()); identical(dotv, dots.read());
    }
    conv.read(true); alpha.read(true); beta_in.read(true); ssm_a.read(true); ssm_dt.read(true);
    std::cout << std::setprecision(17) << "{\"n\":" << n << ",\"key_heads\":16,\"value_heads\":" << vh
              << ",\"dim\":" << dim << ",\"compared\":" << compared << ",\"max_query_error\":" << q_error
              << ",\"max_key_error\":" << k_error << ",\"max_dot_error\":" << dot_error
              << ",\"max_gate_error\":" << gate_error
              << ",\"guards_passed\":true,\"repeat_bitwise\":true,\"empty_noop\":true,\"passed\":true}\n";
    return compared;
}

int main() {
    try {
        checked(cudaSetDevice(0));
        size_t available = 0, total = 0;
        checked(cudaMemGetInfo(&available, &total));
        if (available < (2ull << 30)) throw std::runtime_error("GPU free-memory preflight below2GiB");
        size_t compared = 0;
        for (int vh : {32, 48}) for (int dim : {128, 127}) for (int n : {1, 4, 17})
            compared += exercise(n, vh, dim);
        if (live_bytes != 0) throw std::runtime_error("synthetic buffer leak");
        checked(cudaDeviceSynchronize());
        std::cout << "{\"summary\":true,\"cases\":12,\"compared\":" << compared
                  << ",\"peak_synthetic_device_bytes\":" << peak_bytes
                  << ",\"gpu_free_bytes_after_context\":" << available
                  << ",\"gpu_total_bytes\":" << total << ",\"passed\":true}\n";
        return 0;
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n'; return 1;
    }
}
