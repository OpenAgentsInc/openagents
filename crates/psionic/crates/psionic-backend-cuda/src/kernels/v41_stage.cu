// DeepSeek-V4.1 stage kernels (#12394): the small steps around W4–W6's
// kernels that keep a whole stage step on the device.
//
// A stage step runs as one captured CUDA graph per token. The graph reads
// the token's index within the step from a device counter, so the same
// graph serves every decode position and every token of a prefill:
//
// - `v41_stage_begin` writes the token's position and loads its input: the
//   embedding row (stage 0) or the previous stage's residual streams and
//   pre-mix.
// - `v41_engram_hash` maps the token to its compressed id and hashes the
//   n-grams of every Engram layer, keeping the n-gram history on the device.
// - `v41_stage_load` and `v41_stage_store` copy one token's row between a
//   step-wide staging array and a working buffer.
// - `v41_stage_advance` moves the counter to the next token.
// - `v41_sample` picks the next token from the logits on the device.
// - `v41_candidate_flags` turns a previous stage's candidate block ids into
//   the keep flags layer 20's consumers read.
//
// Nothing here is architecture-specific. Build it without fast math, like
// the other V4.1 kernels.

#include <cuda_runtime.h>
#include <math_constants.h>
#include <stdint.h>

namespace {

constexpr int kThreads = 256;
constexpr int kSampleThreads = 1024;
constexpr int kHashColumns = 24;
constexpr int kHistory = 3;

unsigned blocks_for(long long n, int threads) {
    long long blocks = (n + threads - 1) / threads;
    if (blocks < 1) {
        blocks = 1;
    }
    if (blocks > 65535) {
        blocks = 65535;
    }
    return static_cast<unsigned>(blocks);
}

__device__ __forceinline__ float bf16_word(uint16_t word) {
    return __uint_as_float(static_cast<uint32_t>(word) << 16);
}

__global__ void v41_stage_begin_kernel(
    const int* __restrict__ index,
    const int* __restrict__ base,
    const int* __restrict__ tokens,
    const uint16_t* __restrict__ embed,
    int vocab,
    const float* __restrict__ residual_in,
    const float* __restrict__ pre_in,
    float* __restrict__ residual,
    float* __restrict__ pre,
    int* __restrict__ positions,
    int streams,
    int hidden) {
    const int i = *index;
    if (blockIdx.x == 0 && threadIdx.x == 0) {
        positions[0] = *base + i;
    }
    const long long elements = static_cast<long long>(streams) * hidden;
    const long long stride = static_cast<long long>(gridDim.x) * blockDim.x;
    const long long first = static_cast<long long>(blockIdx.x) * blockDim.x + threadIdx.x;
    if (embed != nullptr) {
        const int token = tokens[i];
        const bool valid = token >= 0 && token < vocab;
        for (long long d = first; d < hidden; d += stride) {
            const float value =
                valid ? bf16_word(embed[static_cast<long long>(token) * hidden + d]) : CUDART_NAN_F;
            for (int k = 0; k < streams; ++k) {
                residual[static_cast<long long>(k) * hidden + d] = value;
            }
        }
        if (first < streams) {
            pre[first] = first == 0 ? 1.0f : 0.0f;
        }
    } else {
        for (long long j = first; j < elements; j += stride) {
            residual[j] = residual_in[static_cast<long long>(i) * elements + j];
        }
        if (first < streams) {
            pre[first] = pre_in[static_cast<long long>(i) * streams + first];
        }
    }
}

__global__ void v41_stage_load_kernel(
    const uint32_t* __restrict__ src,
    uint32_t* __restrict__ dst,
    const int* __restrict__ index,
    int count) {
    const long long offset = static_cast<long long>(*index) * count;
    for (int j = blockIdx.x * blockDim.x + threadIdx.x; j < count; j += gridDim.x * blockDim.x) {
        dst[j] = src[offset + j];
    }
}

__global__ void v41_stage_store_kernel(
    const uint32_t* __restrict__ src,
    uint32_t* __restrict__ dst,
    const int* __restrict__ index,
    int count) {
    const long long offset = static_cast<long long>(*index) * count;
    for (int j = blockIdx.x * blockDim.x + threadIdx.x; j < count; j += gridDim.x * blockDim.x) {
        dst[offset + j] = src[j];
    }
}

__global__ void v41_stage_advance_kernel(int* index) {
    index[0] += 1;
}

// One block. `params` holds, per Engram layer, 4 multipliers, then 24
// primes, then 24 offsets. `history` holds the count (0 to 3) and the last
// three compressed ids, oldest first; position 0 starts a new history.
__global__ void v41_engram_hash_kernel(
    const int* __restrict__ index,
    const int* __restrict__ base,
    const int* __restrict__ tokens,
    const uint32_t* __restrict__ map,
    int vocab,
    int pad,
    int* __restrict__ history,
    const unsigned long long* __restrict__ params,
    int layers,
    int heads,
    long long* __restrict__ ids) {
    __shared__ unsigned long long grams[kHistory + 1];
    __shared__ int valid;
    if (threadIdx.x == 0) {
        const int i = *index;
        const int position = *base + i;
        const int token = tokens[i];
        valid = token >= 0 && token < vocab;
        const int current = valid ? static_cast<int>(map[token]) : pad;
        int count = position == 0 ? 0 : history[0];
        grams[0] = static_cast<unsigned long long>(current);
        for (int shift = 1; shift <= kHistory; ++shift) {
            // History entries older than the sequence start are the pad id,
            // as `NgramHashState::push_compressed` blocks them.
            grams[shift] = shift <= count
                ? static_cast<unsigned long long>(history[1 + count - shift])
                : static_cast<unsigned long long>(pad);
        }
        if (count < kHistory) {
            history[1 + count] = current;
            count += 1;
        } else {
            history[1] = history[2];
            history[2] = history[3];
            history[3] = current;
        }
        history[0] = count;
    }
    __syncthreads();
    const int t = threadIdx.x;
    if (t >= layers * kHashColumns) {
        return;
    }
    const int layer = t / kHashColumns;
    const int col = t % kHashColumns;
    const int gram = col / heads + 1;
    const unsigned long long* p = params + static_cast<long long>(layer) * (4 + 2 * kHashColumns);
    unsigned long long rolling = grams[0] * p[0];
    for (int g = 1; g <= gram; ++g) {
        rolling ^= grams[g] * p[g];
    }
    const unsigned long long prime = p[4 + col];
    const unsigned long long offset = p[4 + kHashColumns + col];
    ids[t] = valid ? static_cast<long long>(rolling % prime + offset) : -1;
}

__device__ __forceinline__ unsigned long long splitmix64(unsigned long long x) {
    x += 0x9E3779B97F4A7C15ull;
    x = (x ^ (x >> 30)) * 0xBF58476D1CE4E5B9ull;
    x = (x ^ (x >> 27)) * 0x94D049BB133111EBull;
    return x ^ (x >> 31);
}

// Block-wide reductions over kSampleThreads threads.
__device__ float block_max(float value, float* scratch) {
    scratch[threadIdx.x] = value;
    __syncthreads();
    for (int s = kSampleThreads / 2; s > 0; s >>= 1) {
        if (threadIdx.x < s) {
            scratch[threadIdx.x] = fmaxf(scratch[threadIdx.x], scratch[threadIdx.x + s]);
        }
        __syncthreads();
    }
    const float out = scratch[0];
    __syncthreads();
    return out;
}

__device__ double block_sum(double value, double* scratch) {
    scratch[threadIdx.x] = value;
    __syncthreads();
    for (int s = kSampleThreads / 2; s > 0; s >>= 1) {
        if (threadIdx.x < s) {
            scratch[threadIdx.x] += scratch[threadIdx.x + s];
        }
        __syncthreads();
    }
    const double out = scratch[0];
    __syncthreads();
    return out;
}

// One block of kSampleThreads. Thread t owns the contiguous ids
// [t * chunk, (t + 1) * chunk). `settings` holds temperature and top-p;
// temperature <= 0 is greedy (ties to the lower id). Any nonfinite logit
// writes -1.
__global__ void v41_sample_kernel(
    const float* __restrict__ logits,
    int vocab,
    const float* __restrict__ settings,
    const unsigned long long* __restrict__ seed,
    const int* __restrict__ positions,
    int* __restrict__ token) {
    __shared__ float fscratch[kSampleThreads];
    __shared__ double dscratch[kSampleThreads];
    __shared__ int iscratch[kSampleThreads];
    const int chunk = (vocab + kSampleThreads - 1) / kSampleThreads;
    const int begin = min(vocab, static_cast<int>(threadIdx.x) * chunk);
    const int end = min(vocab, begin + chunk);
    const float temperature = settings[0];
    const float top_p = settings[1];

    // Maximum and the lowest id that reaches it; NaN or infinity poisons.
    float local_max = -CUDART_INF_F;
    int local_arg = vocab;
    int bad = 0;
    for (int i = begin; i < end; ++i) {
        const float l = logits[i];
        if (!isfinite(l)) {
            bad = 1;
        } else if (l > local_max) {
            local_max = l;
            local_arg = i;
        }
    }
    iscratch[threadIdx.x] = bad;
    __syncthreads();
    for (int s = kSampleThreads / 2; s > 0; s >>= 1) {
        if (threadIdx.x < s) {
            iscratch[threadIdx.x] |= iscratch[threadIdx.x + s];
        }
        __syncthreads();
    }
    const int poisoned = iscratch[0];
    __syncthreads();
    const float m = block_max(local_max, fscratch);
    if (poisoned) {
        if (threadIdx.x == 0) {
            token[0] = -1;
        }
        return;
    }
    iscratch[threadIdx.x] = local_max == m ? local_arg : vocab;
    __syncthreads();
    for (int s = kSampleThreads / 2; s > 0; s >>= 1) {
        if (threadIdx.x < s) {
            iscratch[threadIdx.x] = min(iscratch[threadIdx.x], iscratch[threadIdx.x + s]);
        }
        __syncthreads();
    }
    const int greedy = iscratch[0];
    __syncthreads();
    if (!(temperature > 0.0f)) {
        if (threadIdx.x == 0) {
            token[0] = greedy;
        }
        return;
    }

    // w_i = exp((l_i - m) / T), rounded once from double; the largest is 1.
    auto weight = [&](int i) -> double {
        const float scaled = __fdiv_rn(__fsub_rn(logits[i], m), temperature);
        return static_cast<double>(__double2float_rn(exp(static_cast<double>(scaled))));
    };
    double local = 0.0;
    for (int i = begin; i < end; ++i) {
        local += weight(i);
    }
    const double total = block_sum(local, dscratch);
    // Top-p: the largest threshold whose kept mass reaches top_p of the
    // total, by bisection on [0, 1] to 2^-40.
    double lo = 0.0;
    if (top_p < 1.0f) {
        double hi = 1.0;
        const double want = static_cast<double>(top_p) * total;
        for (int iteration = 0; iteration < 40; ++iteration) {
            const double mid = 0.5 * (lo + hi);
            double kept = 0.0;
            for (int i = begin; i < end; ++i) {
                const double w = weight(i);
                kept += w >= mid ? w : 0.0;
            }
            kept = block_sum(kept, dscratch);
            if (kept >= want) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }
    double mine = 0.0;
    for (int i = begin; i < end; ++i) {
        const double w = weight(i);
        mine += w >= lo ? w : 0.0;
    }
    // Exclusive prefix of the kept mass in id order.
    dscratch[threadIdx.x] = mine;
    __syncthreads();
    for (int s = 1; s < kSampleThreads; s <<= 1) {
        const double add = threadIdx.x >= s ? dscratch[threadIdx.x - s] : 0.0;
        __syncthreads();
        dscratch[threadIdx.x] += add;
        __syncthreads();
    }
    const double kept_total = dscratch[kSampleThreads - 1];
    const double before = dscratch[threadIdx.x] - mine;
    const unsigned long long draw =
        splitmix64(*seed ^ (static_cast<unsigned long long>(positions[0]) * 0xD1B54A32D192ED03ull));
    const double target = static_cast<double>(draw >> 11) * 0x1.0p-53 * kept_total;
    if (threadIdx.x == 0) {
        token[0] = greedy;
    }
    __syncthreads();
    if (target >= before && target < before + mine) {
        double running = before;
        int chosen = -1;
        for (int i = begin; i < end; ++i) {
            const double w = weight(i);
            if (w >= lo) {
                chosen = i;
                running += w;
                if (target < running) {
                    break;
                }
            }
        }
        if (chosen >= 0) {
            token[0] = chosen;
        }
    }
}

// One block: clear `flags`, then set each kept block id of this token's
// row of `blocks` (ids past `capacity` and negative ids are skipped).
__global__ void v41_candidate_flags_kernel(
    const int* __restrict__ blocks,
    const int* __restrict__ index,
    int top_blocks,
    unsigned char* __restrict__ flags,
    int capacity) {
    // Without an index, block t builds token t's flags row (#12464).
    const long long r = index != nullptr ? *index : blockIdx.x;
    if (index == nullptr) {
        flags += static_cast<long long>(blockIdx.x) * capacity;
    }
    for (int b = threadIdx.x; b < capacity; b += blockDim.x) {
        flags[b] = 0;
    }
    __syncthreads();
    const int* row = blocks + r * top_blocks;
    for (int k = threadIdx.x; k < top_blocks; k += blockDim.x) {
        const int b = row[k];
        if (b >= 0 && b < capacity) {
            flags[b] = 1;
        }
    }
}

}  // namespace

extern "C" int psionic_v41_stage_begin(
    const void* index,
    const void* base,
    const void* tokens,
    const void* embed,
    int vocab,
    const void* residual_in,
    const void* pre_in,
    void* residual,
    void* pre,
    void* positions,
    int streams,
    int hidden,
    void* stream) {
    const long long elements = static_cast<long long>(streams) * hidden;
    v41_stage_begin_kernel<<<blocks_for(elements, kThreads), kThreads, 0,
                             static_cast<cudaStream_t>(stream)>>>(
        static_cast<const int*>(index), static_cast<const int*>(base),
        static_cast<const int*>(tokens), static_cast<const uint16_t*>(embed), vocab,
        static_cast<const float*>(residual_in), static_cast<const float*>(pre_in),
        static_cast<float*>(residual), static_cast<float*>(pre), static_cast<int*>(positions),
        streams, hidden);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_stage_copy(
    const void* src, void* dst, const void* index, int count, int store, void* stream) {
    const unsigned grid = blocks_for(count, kThreads);
    if (store) {
        v41_stage_store_kernel<<<grid, kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
            static_cast<const uint32_t*>(src), static_cast<uint32_t*>(dst),
            static_cast<const int*>(index), count);
    } else {
        v41_stage_load_kernel<<<grid, kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
            static_cast<const uint32_t*>(src), static_cast<uint32_t*>(dst),
            static_cast<const int*>(index), count);
    }
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_stage_advance(void* index, void* stream) {
    v41_stage_advance_kernel<<<1, 1, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<int*>(index));
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_engram_hash(
    const void* index,
    const void* base,
    const void* tokens,
    const void* map,
    int vocab,
    int pad,
    void* history,
    const void* params,
    int layers,
    int heads,
    void* ids,
    void* stream) {
    v41_engram_hash_kernel<<<1, 64, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const int*>(index), static_cast<const int*>(base),
        static_cast<const int*>(tokens), static_cast<const uint32_t*>(map), vocab, pad,
        static_cast<int*>(history), static_cast<const unsigned long long*>(params), layers,
        heads, static_cast<long long*>(ids));
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_sample(
    const void* logits,
    int vocab,
    const void* settings,
    const void* seed,
    const void* positions,
    void* token,
    void* stream) {
    v41_sample_kernel<<<1, kSampleThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const float*>(logits), vocab, static_cast<const float*>(settings),
        static_cast<const unsigned long long*>(seed), static_cast<const int*>(positions),
        static_cast<int*>(token));
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_candidate_flags(
    const void* blocks, const void* index, int top_blocks, void* flags, int capacity,
    void* stream) {
    v41_candidate_flags_kernel<<<1, kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const int*>(blocks), static_cast<const int*>(index), top_blocks,
        static_cast<unsigned char*>(flags), capacity);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_candidate_flags_tokens(
    const void* blocks, int tokens, int top_blocks, void* flags, int capacity, void* stream) {
    v41_candidate_flags_kernel<<<tokens, kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const int*>(blocks), nullptr, top_blocks, static_cast<unsigned char*>(flags),
        capacity);
    return static_cast<int>(cudaGetLastError());
}
