// DeepSeek-V4.1 routed-expert cache kernels (#12463).
//
// A stage with an expert cache keeps some of each layer's routed experts in
// device slots and the rest in host memory. A per-layer slot map gives each
// expert its place: a slot index (>= 0), kLaneBus (-1, read from page-locked
// host memory over the bus), or kLaneHost (-2, computed by host CPU threads).
//
// The cached matrix-vector kernel lives in v41_block_matvec.cu. This file has
// the two kernels that hand host-lane experts to the CPU and take their
// outputs back, through a mailbox in page-locked host memory:
//
//   v41_lane_post   after routing: logs the token's expert ids, and when any
//                   selected expert is on the host lane, copies the FP8
//                   activation, ids, and route weights into the mailbox and
//                   publishes a new sequence number.
//   v41_lane_wait   before the expert sum: waits until the host publishes the
//                   same sequence number as done, then copies the host lane's
//                   output rows into the routed output.
//
// The GPU runs the cached and bus-lane experts between the two kernels, so
// host and device work overlap. Both kernels are graph-capturable: they read
// the sequence and pending state from device memory, not from launch
// arguments. A wait that outlives its time limit marks the mailbox failed and
// writes NaN rows, so a stuck host fails the step instead of hanging it.
//
// Design after Project Maya's host mailbox (`mw00/project-maya` 451f9cb6a2,
// src/ds4/ds4_model.cu), reimplemented; MIT.

#include <cuda_runtime.h>
#include <math_constants.h>
#include <stdint.h>

namespace {

constexpr int kThreads = 256;
constexpr int kLaneHost = -2;
constexpr int kMaxSlots = 8;

// Mailbox layout; v41_expert_cache.rs repeats these offsets.
constexpr int kPost = 0;       // u32 sequence the device published
constexpr int kDone = 4;       // u32 sequence the host finished
constexpr int kLayer = 8;      // i32 model layer
constexpr int kMask = 12;      // u32 host-lane slots
constexpr int kError = 16;     // u32 nonzero after a timed-out wait
constexpr int kIds = 32;       // i32[8]
constexpr int kWeights = 64;   // f32[8]
constexpr int kPayload = 128;  // FP8 codes [hidden], then scales [hidden / 32]

__device__ __forceinline__ unsigned long long now_ns() {
    unsigned long long t;
    asm volatile("mov.u64 %0, %%globaltimer;" : "=l"(t));
    return t;
}

// state[0]: last sequence posted; state[1]: the sequence the next wait
// expects, or 0 when this layer posted nothing.
__global__ void __launch_bounds__(kThreads) v41_lane_post_kernel(
    const int* __restrict__ ids,
    const int* __restrict__ slot_map,
    int experts,
    const float* __restrict__ route_weights,
    const uint8_t* __restrict__ act,
    const uint8_t* __restrict__ act_scales,
    int hidden,
    int top_k,
    uint8_t* mailbox,
    unsigned* state,
    int layer,
    const int* __restrict__ index,
    int* ids_log,
    int log_layers,
    int log_slot) {
    __shared__ unsigned mask;
    if (threadIdx.x == 0) {
        unsigned m = 0;
        const long long row = (static_cast<long long>(index[0]) * log_layers + log_slot) * top_k;
        for (int s = 0; s < top_k; ++s) {
            const int id = ids[s];
            ids_log[row + s] = id;
            if (id >= 0 && id < experts && slot_map[id] == kLaneHost) {
                m |= 1u << s;
            }
        }
        mask = m;
    }
    __syncthreads();
    if (mask == 0) {
        if (threadIdx.x == 0) {
            state[1] = 0;
        }
        return;
    }
    // The payload as 4-byte words; hidden is a multiple of 128.
    const uint32_t* codes = reinterpret_cast<const uint32_t*>(act);
    uint32_t* dst = reinterpret_cast<uint32_t*>(mailbox + kPayload);
    for (int i = threadIdx.x; i < hidden / 4; i += kThreads) {
        dst[i] = codes[i];
    }
    uint8_t* scale_dst = mailbox + kPayload + hidden;
    for (int i = threadIdx.x; i < hidden / 32; i += kThreads) {
        scale_dst[i] = act_scales[i];
    }
    __syncthreads();
    if (threadIdx.x == 0) {
        int* mids = reinterpret_cast<int*>(mailbox + kIds);
        float* mweights = reinterpret_cast<float*>(mailbox + kWeights);
        for (int s = 0; s < top_k; ++s) {
            mids[s] = ids[s];
            mweights[s] = route_weights[s];
        }
        *reinterpret_cast<int*>(mailbox + kLayer) = layer;
        *reinterpret_cast<unsigned*>(mailbox + kMask) = mask;
        const unsigned seq = state[0] + 1u;
        state[0] = seq;
        state[1] = seq;
        // Everything above reaches host memory before the sequence does.
        __threadfence_system();
        *reinterpret_cast<volatile unsigned*>(mailbox + kPost) = seq;
        __threadfence_system();
    }
}

__global__ void __launch_bounds__(kThreads) v41_lane_wait_kernel(
    uint8_t* mailbox,
    unsigned* state,
    const float* host_rows,
    float* __restrict__ out,
    int hidden,
    int top_k,
    unsigned long long timeout_ns) {
    __shared__ unsigned failed;
    const unsigned pending = state[1];
    if (pending == 0) {
        return;  // uniform: nothing was posted for this layer
    }
    if (threadIdx.x == 0) {
        const unsigned long long start = now_ns();
        volatile unsigned* done = reinterpret_cast<volatile unsigned*>(mailbox + kDone);
        unsigned f = 0;
        while (*done != pending) {
            if (now_ns() - start > timeout_ns) {
                f = 1;
                *reinterpret_cast<volatile unsigned*>(mailbox + kError) = 1;
                break;
            }
            __nanosleep(200);
        }
        __threadfence_system();
        failed = f;
    }
    __syncthreads();
    const unsigned mask = *reinterpret_cast<volatile unsigned*>(mailbox + kMask);
    const volatile float* rows = host_rows;
    for (int s = 0; s < top_k && s < kMaxSlots; ++s) {
        if ((mask & (1u << s)) == 0) {
            continue;
        }
        for (int d = threadIdx.x; d < hidden; d += kThreads) {
            const long long at = static_cast<long long>(s) * hidden + d;
            out[at] = failed ? CUDART_NAN_F : rows[at];
        }
    }
    __syncthreads();
    if (threadIdx.x == 0) {
        state[1] = 0;
    }
}

}  // namespace

extern "C" int psionic_v41_lane_post(
    const void* ids,
    const void* slot_map,
    int experts,
    const void* route_weights,
    const void* act,
    const void* act_scales,
    int hidden,
    int top_k,
    void* mailbox,
    void* state,
    int layer,
    const void* index,
    void* ids_log,
    int log_layers,
    int log_slot,
    void* stream) {
    v41_lane_post_kernel<<<1, kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<const int*>(ids), static_cast<const int*>(slot_map), experts,
        static_cast<const float*>(route_weights), static_cast<const uint8_t*>(act),
        static_cast<const uint8_t*>(act_scales), hidden, top_k, static_cast<uint8_t*>(mailbox),
        static_cast<unsigned*>(state), layer, static_cast<const int*>(index),
        static_cast<int*>(ids_log), log_layers, log_slot);
    return static_cast<int>(cudaGetLastError());
}

extern "C" int psionic_v41_lane_wait(
    void* mailbox,
    void* state,
    const void* host_rows,
    void* out,
    int hidden,
    int top_k,
    unsigned long long timeout_ns,
    void* stream) {
    v41_lane_wait_kernel<<<1, kThreads, 0, static_cast<cudaStream_t>(stream)>>>(
        static_cast<uint8_t*>(mailbox), static_cast<unsigned*>(state),
        static_cast<const float*>(host_rows), static_cast<float*>(out), hidden, top_k, timeout_ns);
    return static_cast<int>(cudaGetLastError());
}
