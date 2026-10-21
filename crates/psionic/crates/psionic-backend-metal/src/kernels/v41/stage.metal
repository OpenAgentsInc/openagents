// DeepSeek-V4.1 stage kernels on Metal (#12420), a port of
// psionic-backend-cuda/src/kernels/v41_stage.cu (#12394): the small steps
// around the matrix, mixing, and attention kernels that keep a whole stage
// step on the device. A token's index within the step lives in a device
// counter, so one recorded sequence serves every token of a step.
//
// The sampler adds its weights in the software binary64 from common.metal,
// in the CUDA kernel's order, so its sums equal the CUDA kernel's double
// sums bit for bit.

constant constexpr int kStageThreads = 256;
constant constexpr int kSampleThreads = 1024;
constant constexpr int kHashColumns = 24;
constant constexpr int kHistory = 3;

struct StageBeginArgs {
    device const int* index;
    device const int* base;
    device const int* tokens;
    device const ushort* embed;
    device const float* residual_in;
    device const float* pre_in;
    device float* residual;
    device float* pre;
    device int* positions;
    int vocab;
    int streams;
    int hidden;
};

kernel void v41_stage_begin(
    constant StageBeginArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint groups [[threadgroups_per_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    const int i = *a.index;
    if (group == 0 && tid == 0) {
        a.positions[0] = *a.base + i;
    }
    const long elements = long(a.streams) * a.hidden;
    const long stride = long(groups) * kStageThreads;
    const long first = long(group) * kStageThreads + long(tid);
    if (a.embed != nullptr) {
        const int token = a.tokens[i];
        const bool valid = token >= 0 && token < a.vocab;
        for (long d = first; d < a.hidden; d += stride) {
            const float value = valid ? bf16_value(a.embed[long(token) * a.hidden + d]) : V41_NAN;
            for (int k = 0; k < a.streams; ++k) {
                a.residual[long(k) * a.hidden + d] = value;
            }
        }
        if (first < a.streams) {
            a.pre[first] = first == 0 ? 1.0f : 0.0f;
        }
    } else {
        for (long j = first; j < elements; j += stride) {
            a.residual[j] = a.residual_in[long(i) * elements + j];
        }
        if (first < a.streams) {
            a.pre[first] = a.pre_in[long(i) * a.streams + first];
        }
    }
}

struct StageCopyArgs {
    device const uint* src;
    device uint* dst;
    device const int* index;
    int count;
};

kernel void v41_stage_load(
    constant StageCopyArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint groups [[threadgroups_per_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    const long offset = long(*a.index) * a.count;
    for (int j = int(group) * kStageThreads + int(tid); j < a.count; j += int(groups) * kStageThreads) {
        a.dst[j] = a.src[offset + j];
    }
}

kernel void v41_stage_store(
    constant StageCopyArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint groups [[threadgroups_per_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    const long offset = long(*a.index) * a.count;
    for (int j = int(group) * kStageThreads + int(tid); j < a.count; j += int(groups) * kStageThreads) {
        a.dst[offset + j] = a.src[j];
    }
}

struct CopyWordsArgs {
    device const uint* src;
    device uint* dst;
    int count;
};

// Copies `count` words; the Metal form of a device-to-device copy.
kernel void v41_copy_words(
    constant CopyWordsArgs& a [[buffer(0)]],
    uint group [[threadgroup_position_in_grid]],
    uint groups [[threadgroups_per_grid]],
    uint tid [[thread_index_in_threadgroup]]) {
    for (int j = int(group) * kStageThreads + int(tid); j < a.count; j += int(groups) * kStageThreads) {
        a.dst[j] = a.src[j];
    }
}

struct StageAdvanceArgs {
    device int* index;
};

kernel void v41_stage_advance(constant StageAdvanceArgs& a [[buffer(0)]]) {
    a.index[0] += 1;
}

struct EngramHashArgs {
    device const int* index;
    device const int* base;
    device const int* tokens;
    device const uint* map;
    device int* history;
    device const ulong* params;
    device long* ids;
    int vocab;
    int pad;
    int layers;
    int heads;
};

// One group. `params` holds, per Engram layer, 4 multipliers, then 24 primes,
// then 24 offsets. `history` holds the count (0 to 3) and the last three
// compressed ids, oldest first; position 0 starts a new history.
kernel void v41_engram_hash(constant EngramHashArgs& a [[buffer(0)]], uint tid [[thread_index_in_threadgroup]]) {
    threadgroup ulong grams[kHistory + 1];
    threadgroup int valid;
    if (tid == 0) {
        const int i = *a.index;
        const int position = *a.base + i;
        const int token = a.tokens[i];
        valid = token >= 0 && token < a.vocab;
        const int current = valid ? int(a.map[token]) : a.pad;
        int count = position == 0 ? 0 : a.history[0];
        grams[0] = ulong(uint(current));
        for (int shift = 1; shift <= kHistory; ++shift) {
            grams[shift] = shift <= count ? ulong(uint(a.history[1 + count - shift])) : ulong(uint(a.pad));
        }
        if (count < kHistory) {
            a.history[1 + count] = current;
            count += 1;
        } else {
            a.history[1] = a.history[2];
            a.history[2] = a.history[3];
            a.history[3] = current;
        }
        a.history[0] = count;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    const int t = int(tid);
    if (t >= a.layers * kHashColumns) {
        return;
    }
    const int layer = t / kHashColumns;
    const int col = t % kHashColumns;
    const int gram = col / a.heads + 1;
    device const ulong* p = a.params + long(layer) * (4 + 2 * kHashColumns);
    ulong rolling = grams[0] * p[0];
    for (int g = 1; g <= gram; ++g) {
        rolling ^= grams[g] * p[g];
    }
    const ulong prime = p[4 + col];
    const ulong offset = p[4 + kHashColumns + col];
    a.ids[t] = valid ? long(rolling % prime + offset) : -1;
}

inline ulong splitmix64(ulong x) {
    x += 0x9E3779B97F4A7C15ul;
    x = (x ^ (x >> 30)) * 0xBF58476D1CE4E5B9ul;
    x = (x ^ (x >> 27)) * 0x94D049BB133111EBul;
    return x ^ (x >> 31);
}

inline float sample_block_max(float value, threadgroup float* scratch, uint tid) {
    scratch[tid] = value;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = kSampleThreads / 2; s > 0; s >>= 1) {
        if (tid < s) {
            scratch[tid] = fmax(scratch[tid], scratch[tid + s]);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    const float out = scratch[0];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    return out;
}

// The CUDA kernel's double block_sum, in software binary64 and the same tree.
inline ulong sample_block_sum(ulong value, threadgroup ulong* scratch, uint tid) {
    scratch[tid] = value;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = kSampleThreads / 2; s > 0; s >>= 1) {
        if (tid < s) {
            scratch[tid] = sd_add(scratch[tid], scratch[tid + s]);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    const ulong out = scratch[0];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    return out;
}

struct SampleArgs {
    device const float* logits;
    device const float* settings;
    device const ulong* seed;
    device const int* positions;
    device int* token;
    int vocab;
};

// w_i = exp((l_i - m) / T), rounded once to float, as a binary64.
inline ulong sample_weight(device const float* logits, int i, float m, float temperature) {
    const float scaled = fdiv(logits[i] - m, temperature);
    return sd_from_float(exp_once(scaled));
}

// One group of kSampleThreads. Thread t owns the contiguous ids
// [t * chunk, (t + 1) * chunk). Temperature <= 0 is greedy (ties to the lower
// id). Any nonfinite logit writes -1.
kernel void v41_sample(constant SampleArgs& a [[buffer(0)]], uint tid [[thread_index_in_threadgroup]]) {
    threadgroup float fscratch[kSampleThreads];
    threadgroup ulong dscratch[kSampleThreads];
    threadgroup int iscratch[kSampleThreads];
    const int vocab = a.vocab;
    const int chunk = (vocab + kSampleThreads - 1) / kSampleThreads;
    const int begin = min(vocab, int(tid) * chunk);
    const int end = min(vocab, begin + chunk);
    const float temperature = a.settings[0];
    const float top_p = a.settings[1];

    float local_max = -V41_INF;
    int local_arg = vocab;
    int bad = 0;
    for (int i = begin; i < end; ++i) {
        const float l = a.logits[i];
        if (!isfinite(l)) {
            bad = 1;
        } else if (l > local_max) {
            local_max = l;
            local_arg = i;
        }
    }
    iscratch[tid] = bad;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = kSampleThreads / 2; s > 0; s >>= 1) {
        if (tid < s) {
            iscratch[tid] |= iscratch[tid + s];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    const int poisoned = iscratch[0];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    const float m = sample_block_max(local_max, fscratch, tid);
    if (poisoned) {
        if (tid == 0) {
            a.token[0] = -1;
        }
        return;
    }
    iscratch[tid] = local_max == m ? local_arg : vocab;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = kSampleThreads / 2; s > 0; s >>= 1) {
        if (tid < s) {
            iscratch[tid] = min(iscratch[tid], iscratch[tid + s]);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    const int greedy = iscratch[0];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (!(temperature > 0.0f)) {
        if (tid == 0) {
            a.token[0] = greedy;
        }
        return;
    }

    ulong local = 0;
    for (int i = begin; i < end; ++i) {
        local = sd_add(local, sample_weight(a.logits, i, m, temperature));
    }
    const ulong total = sample_block_sum(local, dscratch, tid);
    // Top-p: the largest threshold whose kept mass reaches top_p of the
    // total, by bisection on [0, 1] to 2^-40.
    ulong lo = 0;
    if (top_p < 1.0f) {
        ulong hi = 0x3ff0000000000000ul;  // 1.0
        const ulong want = sd_mul(sd_from_float(top_p), total);
        for (int iteration = 0; iteration < 40; ++iteration) {
            const ulong mid = sd_scale(sd_add(lo, hi), -1);
            ulong kept = 0;
            for (int i = begin; i < end; ++i) {
                const ulong w = sample_weight(a.logits, i, m, temperature);
                kept = sd_add(kept, sd_ge(w, mid) ? w : 0);
            }
            kept = sample_block_sum(kept, dscratch, tid);
            if (sd_ge(kept, want)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    }
    ulong mine = 0;
    for (int i = begin; i < end; ++i) {
        const ulong w = sample_weight(a.logits, i, m, temperature);
        mine = sd_add(mine, sd_ge(w, lo) ? w : 0);
    }
    // Inclusive prefix of the kept mass in id order.
    dscratch[tid] = mine;
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint s = 1; s < kSampleThreads; s <<= 1) {
        const ulong add = tid >= s ? dscratch[tid - s] : 0;
        threadgroup_barrier(mem_flags::mem_threadgroup);
        dscratch[tid] = sd_add(dscratch[tid], add);
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    const ulong kept_total = dscratch[kSampleThreads - 1];
    const ulong before = sd_sub(dscratch[tid], mine);
    const ulong draw = splitmix64(*a.seed ^ (ulong(long(a.positions[0])) * 0xD1B54A32D192ED03ul));
    const ulong target = sd_mul(sd_scale(sd_from_u53(draw >> 11), -53), kept_total);
    if (tid == 0) {
        a.token[0] = greedy;
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (sd_ge(target, before) && sd_lt(target, sd_add(before, mine))) {
        ulong running = before;
        int chosen = -1;
        for (int i = begin; i < end; ++i) {
            const ulong w = sample_weight(a.logits, i, m, temperature);
            if (sd_ge(w, lo)) {
                chosen = i;
                running = sd_add(running, w);
                if (sd_lt(target, running)) {
                    break;
                }
            }
        }
        if (chosen >= 0) {
            a.token[0] = chosen;
        }
    }
}

struct CandidateFlagsArgs {
    device const int* blocks;
    device const int* index;
    device uchar* flags;
    int top_blocks;
    int capacity;
};

// One group: clear `flags`, then set each kept block id of this token's row
// of `blocks` (ids past `capacity` and negative ids are skipped).
kernel void v41_candidate_flags(
    constant CandidateFlagsArgs& a [[buffer(0)]],
    uint tid [[thread_index_in_threadgroup]],
    uint threads [[threads_per_threadgroup]]) {
    for (int b = int(tid); b < a.capacity; b += int(threads)) {
        a.flags[b] = 0;
    }
    threadgroup_barrier(mem_flags::mem_device);
    device const int* row = a.blocks + long(*a.index) * a.top_blocks;
    for (int k = int(tid); k < a.top_blocks; k += int(threads)) {
        const int b = row[k];
        if (b >= 0 && b < a.capacity) {
            a.flags[b] = 1;
        }
    }
}
