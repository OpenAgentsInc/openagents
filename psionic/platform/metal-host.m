/* Device effects for Coax-generated Metal compute kernels (see metal-host.h).
   Build with -fobjc-arc -framework Metal -framework Foundation. */
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#include <unistd.h>
#include "metal-host.h"
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static char log_text[8192];
static void note(const char *fmt, ...) { va_list ap; va_start(ap, fmt); vsnprintf(log_text, sizeof log_text, fmt, ap); va_end(ap); }
const char *pm_log(void) { return log_text; }

static id<MTLDevice> device;
static id<MTLCommandQueue> queue;
/* Every live buffer: a kernel reaches buffers by address, so each encoder
   makes all of them resident. */
static NSMutableArray<id<MTLBuffer>> *live;
static NSMutableArray<id<MTLBuffer>> *live_ro;
static id<MTLBuffer> shared_errors;
static __unsafe_unretained id<MTLResource> *live_cache;
static NSUInteger live_cache_cap;
static __unsafe_unretained id<MTLResource> *live_ro_cache;
static NSUInteger live_ro_cache_cap;
static int live_dirty = 1;

/* Metal 4 fast path (macOS 26+): persistent residency set, double-buffered zero-copy
   argument rings and command allocators/buffers/argument tables (bypassed if PM_METAL3=1 or PM_PASS_PROF). */
static int m4_enabled = 0;
#if defined(__MAC_26_0)
API_AVAILABLE(macos(26.0)) static id<MTL4CommandQueue> m4_queue;
API_AVAILABLE(macos(26.0)) static id<MTL4CommandAllocator> m4_alloc[2];
API_AVAILABLE(macos(26.0)) static id<MTL4CommandBuffer> m4_cb[2];
API_AVAILABLE(macos(26.0)) static id<MTL4ArgumentTable> m4_at[2];
API_AVAILABLE(macos(15.0)) static id<MTLResidencySet> m4_res;
static id<MTLBuffer> m4_arg_ring[2];
static id<MTLBuffer> m4_errors[2];
static uint8_t *m4_arg_ptr[2];
static uint64_t m4_arg_gpu[2];
static size_t m4_arg_cap;
static size_t m4_arg_off[2];
static double m4_t_submit[2];
static double m4_t_last_done = 0.0;
static id<MTLSharedEvent> m4_ev;
static uint64_t m4_ev_val = 0;
static int m4_res_dirty = 1;
typedef void (*ImpM4SetAddr)(id, SEL, MTLGPUAddress, NSUInteger);
typedef void (*ImpM4Barrier)(id, SEL, uint64_t, uint64_t, uint64_t);
static ImpM4SetAddr m4_fn_set_addr;
#endif

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }

PmStatus pm_memory(PmDeviceInfo *info) {
    if (!device) return PM_ABSENT;
    info->working_set_bytes = (size_t)device.recommendedMaxWorkingSetSize;
    info->max_buffer_bytes = (size_t)device.maxBufferLength;
    info->allocated_bytes = (size_t)device.currentAllocatedSize;
    return PM_OK;
}

PmStatus pm_open(PmDeviceInfo *info) {
    @autoreleasepool {
        if (!device) {
            device = MTLCreateSystemDefaultDevice();
            if (!device) { note("no Metal device"); return PM_ABSENT; }
            queue = [device newCommandQueueWithMaxCommandBufferCount:64];
            live = [NSMutableArray array];
            live_ro = [NSMutableArray array];
            shared_errors = [device newBufferWithLength:8 options:MTLResourceStorageModeShared | MTLResourceHazardTrackingModeUntracked];
#if defined(__MAC_26_0)
            const char *m3 = getenv("PM_METAL3");
            if (@available(macOS 26.0, *)) {
                if (!m3 || m3[0] == '0') {
                    NSError *err = nil;
                    m4_queue = [device newMTL4CommandQueue];
                    MTL4ArgumentTableDescriptor *atd = [MTL4ArgumentTableDescriptor new];
                    atd.maxBufferBindCount = 4;
                    MTLResidencySetDescriptor *rsd = [MTLResidencySetDescriptor new];
                    rsd.initialCapacity = 512;
                    m4_res = m4_queue ? [device newResidencySetWithDescriptor:rsd error:&err] : nil;
                    m4_arg_cap = 8 * 1024 * 1024;
                    m4_ev = m4_res ? [device newSharedEvent] : nil;
                    int ok = (m4_ev != nil);
                    for (int s = 0; s < 2 && ok; s++) {
                        m4_alloc[s] = [device newCommandAllocator];
                        m4_cb[s] = [device newCommandBuffer];
                        m4_at[s] = [device newArgumentTableWithDescriptor:atd error:&err];
                        m4_arg_ring[s] = [device newBufferWithLength:m4_arg_cap options:MTLResourceStorageModeShared | MTLResourceHazardTrackingModeUntracked];
                        m4_errors[s] = [device newBufferWithLength:8 options:MTLResourceStorageModeShared | MTLResourceHazardTrackingModeUntracked];
                        if (!m4_alloc[s] || !m4_cb[s] || !m4_at[s] || !m4_arg_ring[s] || !m4_errors[s]) ok = 0;
                    }
                    if (ok) {
                        [m4_res addAllocation:shared_errors];
                        for (int s = 0; s < 2; s++) {
                            m4_arg_ptr[s] = (uint8_t *)m4_arg_ring[s].contents;
                            m4_arg_gpu[s] = m4_arg_ring[s].gpuAddress;
                            memset(m4_errors[s].contents, 0, 8);
                            [m4_res addAllocation:m4_arg_ring[s]];
                            [m4_res addAllocation:m4_errors[s]];
                        }
                        [m4_res commit];
                        [m4_queue addResidencySet:m4_res];
                        m4_res_dirty = 0;
                        m4_fn_set_addr = (ImpM4SetAddr)[(NSObject *)m4_at[0] methodForSelector:@selector(setAddress:atIndex:)];
                        m4_enabled = 1;
                    }
                }
            }
#endif
        }
        memset(info, 0, sizeof *info);
        snprintf(info->name, sizeof info->name, "%s", device.name.UTF8String);
        info->unified_memory = device.hasUnifiedMemory ? 1 : 0;
        for (int n = 10; n >= 1; n--) {
            if ([device supportsFamily:(MTLGPUFamily)(MTLGPUFamilyApple1 + n - 1)]) { info->apple_family = n; break; }
        }
        return pm_memory(info);
    }
}

static PmStatus compile_impl(const char *source, size_t length, const char *name, int fast_math, PmModule *module, double *seconds) {
    @autoreleasepool {
        if (!device) return PM_ABSENT;
        double t0 = now();
        NSString *text = [[NSString alloc] initWithBytes:source length:length encoding:NSUTF8StringEncoding];
        if (!text) { note("%s: not UTF-8", name); return PM_ARGUMENT; }
        MTLCompileOptions *options = [MTLCompileOptions new];
        if (@available(macOS 15.0, *)) {
            if (fast_math) {
                /* Safe IEEE arithmetic preserves bit-exact parity & Q4S precision; Fast FP functions accelerate exp/tanh/rsqrt. */
                options.mathMode = MTLMathModeSafe;
                options.mathFloatingPointFunctions = MTLMathFloatingPointFunctionsFast;
                options.preprocessorMacros = @{ @"COAX_METAL_FAST_MATH": @1 };
            } else {
                /* IEEE arithmetic, one rounding per operation, precise math functions. */
                options.mathMode = MTLMathModeSafe;
                options.mathFloatingPointFunctions = MTLMathFloatingPointFunctionsPrecise;
            }
        }
        /* Metal 4 (macOS 26) for the tensor ops of am_matmul_bf16; 3.1 otherwise. */
        if (@available(macOS 26.0, *)) options.languageVersion = MTLLanguageVersion4_0;
        else options.languageVersion = MTLLanguageVersion3_1;
        NSError *error = nil;
        id<MTLLibrary> library = [device newLibraryWithSource:text options:options error:&error];
        if (!library) { note("%s: %s", name, error.localizedDescription.UTF8String); return PM_COMPILE; }
        *module = (__bridge_retained void *)library;
        if (seconds) *seconds = now() - t0;
        return PM_OK;
    }
}

PmStatus pm_compile(const char *source, size_t length, const char *name, PmModule *module, double *seconds) {
    return compile_impl(source, length, name, 0, module, seconds);
}

PmStatus pm_compile_fast(const char *source, size_t length, const char *name, PmModule *module, double *seconds) {
    return compile_impl(source, length, name, 1, module, seconds);
}

typedef struct {
    void *buf; /* __bridge_retained id<MTLBuffer> */
    uint64_t gpu_addr;
    NSUInteger length;
    void *contents;
} PmBufImpl;

typedef struct {
    void *pso; /* __bridge_retained id<MTLComputePipelineState> */
    NSUInteger max_threads;
    char name[64];
} PmKernelImpl;

typedef void (*ImpSetPSO)(id, SEL, id<MTLComputePipelineState>);
typedef void (*ImpSetBytes)(id, SEL, const void *, NSUInteger, NSUInteger);
typedef void (*ImpDispatchTG)(id, SEL, MTLSize, MTLSize);
typedef void (*ImpBarrier)(id, SEL, MTLBarrierScope);

typedef struct {
    const char *name;
    int64_t items;
    int lanes;
} PmProfRec;

typedef struct {
    void *commands; /* __bridge_retained id<MTLCommandBuffer> */
    void *encoder;  /* __bridge_retained id<MTLComputeCommandEncoder> */
    void *errors;   /* __bridge id<MTLBuffer> (shared_errors) */
    void *last_pipeline;
    ImpSetPSO fn_set_pso;
    ImpSetBytes fn_set_bytes;
    ImpDispatchTG fn_dispatch_tg;
    ImpDispatchTG fn_dispatch_threads;
    ImpBarrier fn_barrier;
    /* Optional hardware timestamp counter profiling (PM_PASS_PROF=1: per-kernel, 2: per-wave). */
    int prof_mode;
    int prof_count;
    int prof_cap;
    void *prof_csb; /* __bridge_retained id<MTLCounterSampleBuffer> */
    PmProfRec *prof_recs;
    MTLDispatchType dtype;
    int is_m4;
    int m4_slot;
#if defined(__MAC_26_0)
    ImpM4Barrier fn_m4_barrier;
#endif
} PmBatchImpl;

PmStatus pm_kernel(PmModule module, const char *name, PmKernel *kernel) {
    @autoreleasepool {
        id<MTLLibrary> library = (__bridge id<MTLLibrary>)module;
        id<MTLFunction> function = [library newFunctionWithName:[NSString stringWithUTF8String:name]];
        if (!function) { note("no kernel %s", name); return PM_ARGUMENT; }
        NSError *error = nil;
        id<MTLComputePipelineState> pipeline = [device newComputePipelineStateWithFunction:function error:&error];
        if (!pipeline) { note("%s: %s", name, error.localizedDescription.UTF8String); return PM_COMPILE; }
        if (getenv("PM_VERBOSE")) fprintf(stderr, "kernel %s: %lu threads per group, simd width %lu\n", name, (unsigned long)pipeline.maxTotalThreadsPerThreadgroup, (unsigned long)pipeline.threadExecutionWidth);
        PmKernelImpl *k = (PmKernelImpl *)malloc(sizeof *k);
        if (!k) return PM_MEMORY;
        k->max_threads = pipeline.maxTotalThreadsPerThreadgroup;
        k->pso = (__bridge_retained void *)pipeline;
        snprintf(k->name, sizeof k->name, "%s", name);
        *kernel = k;
        return PM_OK;
    }
}

PmStatus pm_kernel_free(PmKernel kernel) {
    if (!kernel) return PM_OK;
    PmKernelImpl *k = (PmKernelImpl *)kernel;
    id<MTLComputePipelineState> object = (__bridge_transfer id<MTLComputePipelineState>)k->pso;
    (void)object;
    free(k);
    return PM_OK;
}
PmStatus pm_unload(PmModule module) {
    if (!module) return PM_OK;
    id<MTLLibrary> object = (__bridge_transfer id<MTLLibrary>)module;
    (void)object;
    return PM_OK;
}

static PmStatus wrap_buf_impl(id<MTLBuffer> b, int ro, PmBuffer *buffer) {
    PmBufImpl *bi = (PmBufImpl *)malloc(sizeof *bi);
    if (!bi) return PM_MEMORY;
    bi->gpu_addr = b.gpuAddress;
    bi->length = b.length;
    bi->contents = b.contents;
    if (ro) [live_ro addObject:b];
    else [live addObject:b];
    live_dirty = 1;
#if defined(__MAC_26_0)
    if (@available(macOS 26.0, *)) { if (m4_enabled) { [m4_res addAllocation:b]; m4_res_dirty = 1; } }
#endif
    bi->buf = (__bridge_retained void *)b;
    *buffer = bi;
    return PM_OK;
}

static PmStatus alloc_buffer(size_t bytes, PmBuffer *buffer, int zero) {
    @autoreleasepool {
        if (!device) return PM_ABSENT;
        if (bytes == 0) bytes = 16;
        id<MTLBuffer> b = [device newBufferWithLength:bytes options:MTLResourceStorageModeShared | MTLResourceHazardTrackingModeUntracked];
        if (!b) { note("cannot allocate %zu bytes", bytes); return PM_MEMORY; }
        if (zero) memset(b.contents, 0, bytes);
        return wrap_buf_impl(b, 0, buffer);
    }
}
PmStatus pm_alloc(size_t bytes, PmBuffer *buffer) { return alloc_buffer(bytes, buffer, 1); }
PmStatus pm_wrap(void *bytes, size_t length, PmBuffer *buffer) {
    @autoreleasepool {
        if (!device) return PM_ABSENT;
        size_t page = (size_t)getpagesize();
        if (!bytes || length == 0 || (uintptr_t)bytes % page || length % page) { note("pm_wrap needs page-aligned memory"); return PM_ARGUMENT; }
        id<MTLBuffer> b = [device newBufferWithBytesNoCopy:bytes length:length options:MTLResourceStorageModeShared | MTLResourceHazardTrackingModeUntracked deallocator:nil];
        if (!b) { note("cannot wrap %zu bytes", length); return PM_MEMORY; }
        return wrap_buf_impl(b, 1, buffer);
    }
}
PmStatus pm_alloc_unset(size_t bytes, PmBuffer *buffer) { return alloc_buffer(bytes, buffer, 0); }

void pm_mark_readonly(PmBuffer buffer) {
    if (!buffer) return;
    PmBufImpl *bi = (PmBufImpl *)buffer;
    id<MTLBuffer> b = (__bridge id<MTLBuffer>)bi->buf;
    if ([live containsObject:b]) {
        [live removeObjectIdenticalTo:b];
        [live_ro addObject:b];
        live_dirty = 1;
    }
}

PmStatus pm_free(PmBuffer buffer) {
    if (!buffer) return PM_OK;
    PmBufImpl *bi = (PmBufImpl *)buffer;
    id<MTLBuffer> b = (__bridge_transfer id<MTLBuffer>)bi->buf;
    [live removeObjectIdenticalTo:b];
    [live_ro removeObjectIdenticalTo:b];
    live_dirty = 1;
#if defined(__MAC_26_0)
    if (@available(macOS 26.0, *)) { if (m4_enabled) { [m4_res removeAllocation:b]; m4_res_dirty = 1; } }
#endif
    free(bi);
    return PM_OK;
}

void *pm_contents(PmBuffer buffer) { return buffer ? ((PmBufImpl *)buffer)->contents : NULL; }

static int prof_batch_seq = 0;

static id<MTLComputeCommandEncoder> prof_open_pass(PmBatchImpl *b, id<MTLCommandBuffer> cmds, int idx) {
    MTLComputePassDescriptor *desc = [MTLComputePassDescriptor computePassDescriptor];
    desc.dispatchType = b->dtype;
    if (b->prof_csb && idx < b->prof_cap) {
        desc.sampleBufferAttachments[0].sampleBuffer = (__bridge id<MTLCounterSampleBuffer>)b->prof_csb;
        desc.sampleBufferAttachments[0].startOfEncoderSampleIndex = (NSUInteger)(idx * 2);
        desc.sampleBufferAttachments[0].endOfEncoderSampleIndex = (NSUInteger)(idx * 2 + 1);
    }
    id<MTLComputeCommandEncoder> enc = [cmds computeCommandEncoderWithDescriptor:desc];
    NSUInteger n = live.count;
    NSUInteger n_ro = live_ro.count;
    if (n) [enc useResources:live_cache count:n usage:MTLResourceUsageRead | MTLResourceUsageWrite];
    if (n_ro) [enc useResources:live_ro_cache count:n_ro usage:MTLResourceUsageRead];
    [enc setBuffer:shared_errors offset:0 atIndex:1];
    b->last_pipeline = NULL;
    NSObject *enc_obj = (NSObject *)enc;
    b->fn_set_pso = (ImpSetPSO)[enc_obj methodForSelector:@selector(setComputePipelineState:)];
    b->fn_set_bytes = (ImpSetBytes)[enc_obj methodForSelector:@selector(setBytes:length:atIndex:)];
    b->fn_dispatch_tg = (ImpDispatchTG)[enc_obj methodForSelector:@selector(dispatchThreadgroups:threadsPerThreadgroup:)];
    b->fn_dispatch_threads = (ImpDispatchTG)[enc_obj methodForSelector:@selector(dispatchThreads:threadsPerThreadgroup:)];
    b->fn_barrier = (ImpBarrier)[enc_obj methodForSelector:@selector(memoryBarrierWithScope:)];
    return enc;
}

static PmStatus begin_impl(PmBatch *batch, MTLDispatchType dtype, int slot) {
    @autoreleasepool {
        if (!device) return PM_ABSENT;
        PmBatchImpl *b = (PmBatchImpl *)calloc(1, sizeof *b);
        if (!b) return PM_MEMORY;
        b->dtype = dtype;
        b->m4_slot = (slot & 1);
        int pmode = 0;
        const char *penv = getenv("PM_PASS_PROF");
        if (penv && penv[0] != '0') {
            int seq = ++prof_batch_seq;
            int target = getenv("PM_PASS_PROF_STEP") ? atoi(getenv("PM_PASS_PROF_STEP")) : 10;
            if (seq == target) pmode = atoi(penv);
        }
        if (!shared_errors) {
            shared_errors = [device newBufferWithLength:8 options:MTLResourceStorageModeShared | MTLResourceHazardTrackingModeUntracked];
        }
#if defined(__MAC_26_0)
        if (m4_enabled && !pmode && dtype == MTLDispatchTypeConcurrent) {
            if (@available(macOS 26.0, *)) {
                int sl = b->m4_slot;
                memset(m4_errors[sl].contents, 0, 8);
                if (m4_res_dirty) {
                    [m4_res commit];
                    m4_res_dirty = 0;
                }
                m4_arg_off[sl] = 0;
                [m4_alloc[sl] reset];
                [m4_cb[sl] beginCommandBufferWithAllocator:m4_alloc[sl]];
                id<MTL4ComputeCommandEncoder> enc4 = [m4_cb[sl] computeCommandEncoder];
                m4_fn_set_addr(m4_at[sl], @selector(setAddress:atIndex:), m4_errors[sl].gpuAddress, 1);
                [enc4 setArgumentTable:m4_at[sl]];
                NSObject *enc_obj = (NSObject *)enc4;
                b->fn_set_pso = (ImpSetPSO)[enc_obj methodForSelector:@selector(setComputePipelineState:)];
                b->fn_dispatch_tg = (ImpDispatchTG)[enc_obj methodForSelector:@selector(dispatchThreadgroups:threadsPerThreadgroup:)];
                b->fn_dispatch_threads = (ImpDispatchTG)[enc_obj methodForSelector:@selector(dispatchThreads:threadsPerThreadgroup:)];
                b->fn_m4_barrier = (ImpM4Barrier)[enc_obj methodForSelector:@selector(barrierAfterEncoderStages:beforeEncoderStages:visibilityOptions:)];
                b->is_m4 = 1;
                b->encoder = (__bridge_retained void *)enc4;
                b->errors = (__bridge void *)m4_errors[sl];
                *batch = b;
                return PM_OK;
            }
        }
#endif
        memset(shared_errors.contents, 0, 8);
        id<MTLCommandBuffer> cmds = [queue commandBufferWithUnretainedReferences];
        NSUInteger n = live.count;
        NSUInteger n_ro = live_ro.count;
        if (live_dirty) {
            if (n > live_cache_cap) {
                free(live_cache);
                live_cache_cap = n * 2;
                live_cache = (__unsafe_unretained id<MTLResource> *)malloc(live_cache_cap * sizeof(id<MTLResource>));
            }
            for (NSUInteger i = 0; i < n; i++) live_cache[i] = live[i];
            if (n_ro > live_ro_cache_cap) {
                free(live_ro_cache);
                live_ro_cache_cap = n_ro * 2;
                live_ro_cache = (__unsafe_unretained id<MTLResource> *)malloc(live_ro_cache_cap * sizeof(id<MTLResource>));
            }
            for (NSUInteger i = 0; i < n_ro; i++) live_ro_cache[i] = live_ro[i];
            live_dirty = 0;
        }
        id<MTLComputeCommandEncoder> enc = nil;
        if (pmode) {
            b->prof_mode = pmode;
            b->prof_cap = 2048;
            b->prof_recs = (PmProfRec *)calloc((size_t)b->prof_cap, sizeof(PmProfRec));
            id<MTLCounterSet> tsSet = nil;
            for (id<MTLCounterSet> cs in device.counterSets) {
                if ([cs.name isEqualToString:MTLCommonCounterSetTimestamp]) { tsSet = cs; break; }
            }
            if (tsSet) {
                MTLCounterSampleBufferDescriptor *cd = [MTLCounterSampleBufferDescriptor new];
                cd.counterSet = tsSet;
                cd.sampleCount = (NSUInteger)(b->prof_cap * 2);
                cd.storageMode = MTLStorageModeShared;
                NSError *cerr = nil;
                id<MTLCounterSampleBuffer> csb = [device newCounterSampleBufferWithDescriptor:cd error:&cerr];
                if (csb) b->prof_csb = (__bridge_retained void *)csb;
            }
            if (pmode == 2) {
                enc = prof_open_pass(b, cmds, 0);
                b->prof_recs[0].name = "wave_0";
                b->prof_count = 1;
            }
        }
        if (!pmode) {
            enc = [cmds computeCommandEncoderWithDispatchType:dtype];
            if (n) {
                [enc useResources:live_cache count:n usage:MTLResourceUsageRead | MTLResourceUsageWrite];
            }
            if (n_ro) {
                [enc useResources:live_ro_cache count:n_ro usage:MTLResourceUsageRead];
            }
            [enc setBuffer:shared_errors offset:0 atIndex:1];
            NSObject *enc_obj = (NSObject *)enc;
            b->fn_set_pso = (ImpSetPSO)[enc_obj methodForSelector:@selector(setComputePipelineState:)];
            b->fn_set_bytes = (ImpSetBytes)[enc_obj methodForSelector:@selector(setBytes:length:atIndex:)];
            b->fn_dispatch_tg = (ImpDispatchTG)[enc_obj methodForSelector:@selector(dispatchThreadgroups:threadsPerThreadgroup:)];
            b->fn_dispatch_threads = (ImpDispatchTG)[enc_obj methodForSelector:@selector(dispatchThreads:threadsPerThreadgroup:)];
            b->fn_barrier = (ImpBarrier)[enc_obj methodForSelector:@selector(memoryBarrierWithScope:)];
        }
        b->commands = (__bridge_retained void *)cmds;
        b->encoder = enc ? (__bridge_retained void *)enc : NULL;
        b->errors = (__bridge void *)shared_errors;
        *batch = b;
        return PM_OK;
    }
}

PmStatus pm_begin(PmBatch *batch) { return begin_impl(batch, MTLDispatchTypeSerial, 0); }
PmStatus pm_begin_concurrent(PmBatch *batch) { return begin_impl(batch, MTLDispatchTypeConcurrent, 0); }
int pm_is_m4(void) { return m4_enabled; }
PmStatus pm_begin_concurrent_slot(PmBatch *batch, int slot) { return begin_impl(batch, MTLDispatchTypeConcurrent, slot); }
void pm_patch_first_arg_int(PmBatch batch, int64_t value) {
    PmBatchImpl *b = (PmBatchImpl *)batch;
#if defined(__MAC_26_0)
    if (b && b->is_m4) {
        memcpy(m4_arg_ptr[b->m4_slot] + 8, &value, 8);
    }
#else
    (void)b; (void)value;
#endif
}
void pm_abort(PmBatch batch) {
    if (!batch) return;
    @autoreleasepool {
        PmBatchImpl *b = (PmBatchImpl *)batch;
#if defined(__MAC_26_0)
        if (b->is_m4) {
            if (@available(macOS 26.0, *)) {
                int sl = b->m4_slot;
                id<MTL4ComputeCommandEncoder> enc4 = (__bridge_transfer id<MTL4ComputeCommandEncoder>)b->encoder;
                [enc4 endEncoding];
                [m4_cb[sl] endCommandBuffer];
                /* Detach uncommitted command buffer from m4_alloc[sl] before next begin_impl resets the allocator. */
                m4_cb[sl] = [device newCommandBuffer];
                free(b);
                return;
            }
        }
#endif
        id<MTLComputeCommandEncoder> enc = b->encoder ? (__bridge_transfer id<MTLComputeCommandEncoder>)b->encoder : nil;
        id<MTLCommandBuffer> cmds = (__bridge_transfer id<MTLCommandBuffer>)b->commands;
        id<MTLCounterSampleBuffer> prof_csb = b->prof_csb ? (__bridge_transfer id<MTLCounterSampleBuffer>)b->prof_csb : nil;
        if (enc) [enc endEncoding];
        (void)cmds; (void)prof_csb;
        free(b->prof_recs);
        free(b);
    }
}
void pm_barrier(PmBatch batch) {
    PmBatchImpl *b = (PmBatchImpl *)batch;
#if defined(__MAC_26_0)
    if (b->is_m4) {
        if (@available(macOS 26.0, *)) {
            b->fn_m4_barrier((__bridge id)b->encoder, @selector(barrierAfterEncoderStages:beforeEncoderStages:visibilityOptions:),
                             MTLStageDispatch, MTLStageDispatch, MTL4VisibilityOptionDevice);
        }
        return;
    }
#endif
    if (b->prof_mode == 1) return;
    if (b->prof_mode == 2 && b->prof_count < b->prof_cap) {
        if (b->encoder) {
            id<MTLComputeCommandEncoder> old_enc = (__bridge_transfer id<MTLComputeCommandEncoder>)b->encoder;
            [old_enc endEncoding];
            b->encoder = NULL;
        }
        id<MTLCommandBuffer> cmds = (__bridge id<MTLCommandBuffer>)b->commands;
        int idx = b->prof_count++;
        id<MTLComputeCommandEncoder> new_enc = prof_open_pass(b, cmds, idx);
        b->encoder = (__bridge_retained void *)new_enc;
        return;
    }
    b->fn_barrier((__bridge id)b->encoder, @selector(memoryBarrierWithScope:), MTLBarrierScopeBuffers);
}

PmStatus pm_launch(PmBatch batch, PmKernel kernel, int64_t items, int lanes, int block, const PmArg *args, int count) {
    PmBatchImpl *b = (PmBatchImpl *)batch;
    PmKernelImpl *k = (PmKernelImpl *)kernel;
    if (items < 0 || (lanes != 1 && (lanes % 32 != 0 || lanes > 1024)) || count < 0 || count > 32) { note("malformed launch"); return PM_ARGUMENT; }
    if (items == 0) return PM_OK;
    uint8_t stack_bytes[8 + 32 * 16];
    uint8_t *dst = stack_bytes;
#if defined(__MAC_26_0)
    size_t ring_off = 0;
    if (b->is_m4) {
        int sl = b->m4_slot;
        ring_off = (m4_arg_off[sl] + 15) & ~(size_t)15;
        if (ring_off + sizeof(stack_bytes) > m4_arg_cap) { note("Metal 4 argument ring overflow"); return PM_MEMORY; }
        dst = m4_arg_ptr[sl] + ring_off;
    }
#endif
    size_t at = 8;
    memcpy(dst, &items, 8);
    for (int i = 0; i < count; i++) {
        const PmArg *a = &args[i];
        switch (a->kind) {
            case PM_ARG_INT:
                memcpy(dst + at, &a->i, 8);
                at += 8;
                break;
            case PM_ARG_U32: {
                uint64_t v = (uint32_t)a->i;
                memcpy(dst + at, &v, 8);
                at += 8;
                break;
            }
            case PM_ARG_F32: {
                float v = (float)a->f;
                uint64_t z = 0;
                memcpy(&z, &v, 4);
                memcpy(dst + at, &z, 8);
                at += 8;
                break;
            }
            case PM_ARG_F64: {
                float hi = (float)a->f, lo = (float)(a->f - (double)hi);
                memcpy(dst + at, &hi, 4);
                memcpy(dst + at + 4, &lo, 4);
                at += 8;
                break;
            }
            case PM_ARG_VIEW: {
                PmBufImpl *bi = (PmBufImpl *)a->buffer;
                if (!bi) { note("argument %d: a view without a buffer", i); return PM_ARGUMENT; }
                if (a->offset < 0 || a->count < 0 || (NSUInteger)a->offset > bi->length ||
                    (NSUInteger)a->count > bi->length - (NSUInteger)a->offset) {
                    note("argument %d: a view outside its buffer", i); return PM_ARGUMENT;
                }
                uint64_t address = bi->gpu_addr + (uint64_t)a->offset;
                memcpy(dst + at, &address, 8);
                memcpy(dst + at + 8, &a->count, 8);
                at += 16;
                break;
            }
            default: note("argument %d: unknown kind", i); return PM_ARGUMENT;
        }
    }
    if (b->prof_mode == 1 && b->prof_count < b->prof_cap) {
        if (b->encoder) {
            id<MTLComputeCommandEncoder> old_enc = (__bridge_transfer id<MTLComputeCommandEncoder>)b->encoder;
            [old_enc endEncoding];
            b->encoder = NULL;
        }
        id<MTLCommandBuffer> cmds = (__bridge id<MTLCommandBuffer>)b->commands;
        int idx = b->prof_count++;
        b->prof_recs[idx].name = k->name;
        b->prof_recs[idx].items = items;
        b->prof_recs[idx].lanes = lanes;
        id<MTLComputeCommandEncoder> new_enc = prof_open_pass(b, cmds, idx);
        b->encoder = (__bridge_retained void *)new_enc;
    } else if (b->prof_mode == 2 && b->prof_count > 0 && b->prof_count <= b->prof_cap) {
        if (!b->prof_recs[b->prof_count - 1].name) {
            b->prof_recs[b->prof_count - 1].name = k->name;
            b->prof_recs[b->prof_count - 1].items = items;
            b->prof_recs[b->prof_count - 1].lanes = lanes;
        }
    }
    id enc = (__bridge id)b->encoder;
    if (b->last_pipeline != kernel) {
        b->fn_set_pso(enc, @selector(setComputePipelineState:), (__bridge id<MTLComputePipelineState>)k->pso);
        b->last_pipeline = kernel;
    }
#if defined(__MAC_26_0)
    if (b->is_m4) {
        if (@available(macOS 26.0, *)) {
            int sl = b->m4_slot;
            m4_arg_off[sl] = ring_off + at;
            m4_fn_set_addr(m4_at[sl], @selector(setAddress:atIndex:), m4_arg_gpu[sl] + (uint64_t)ring_off, 0);
        }
    } else
#endif
    {
        b->fn_set_bytes(enc, @selector(setBytes:length:atIndex:), dst, at, 0);
    }
    if (lanes > 32) {
        b->fn_dispatch_tg(enc, @selector(dispatchThreadgroups:threadsPerThreadgroup:),
                          MTLSizeMake((NSUInteger)items, 1, 1), MTLSizeMake((NSUInteger)lanes, 1, 1));
        return PM_OK;
    }
    NSUInteger threads = (NSUInteger)items * (NSUInteger)lanes;
    NSUInteger limit = k->max_threads;
    NSUInteger group = block > 0 ? (NSUInteger)block : 128;
    if (threads < group) group = ((threads + 31) / 32) * 32;
    if (group > limit) group = limit;
    group = group / 32 * 32;
    if (group == 0) { note("pipeline allows fewer than 32 threads per group"); return PM_ARGUMENT; }
    b->fn_dispatch_threads(enc, @selector(dispatchThreads:threadsPerThreadgroup:),
                           MTLSizeMake(threads, 1, 1), MTLSizeMake(group, 1, 1));
    return PM_OK;
}

PmStatus pm_launch_indirect(PmBatch batch, PmKernel kernel, int64_t max_items,
                            PmBuffer indirect_buf, int64_t indirect_offset, int lanes,
                            const PmArg *args, int count) {
    PmBatchImpl *b = (PmBatchImpl *)batch;
    PmKernelImpl *k = (PmKernelImpl *)kernel;
    PmBufImpl *ibi = (PmBufImpl *)indirect_buf;
    if (!ibi || max_items < 0 || lanes <= 32 || lanes % 32 != 0 || lanes > 1024 || count < 0 || count > 32) {
        note("malformed indirect launch");
        return PM_ARGUMENT;
    }
    if (max_items == 0) return PM_OK;
    uint8_t stack_bytes[8 + 32 * 16];
    uint8_t *dst = stack_bytes;
#if defined(__MAC_26_0)
    size_t ring_off = 0;
    if (b->is_m4) {
        int sl = b->m4_slot;
        ring_off = (m4_arg_off[sl] + 15) & ~(size_t)15;
        if (ring_off + sizeof(stack_bytes) > m4_arg_cap) { note("Metal 4 argument ring overflow"); return PM_MEMORY; }
        dst = m4_arg_ptr[sl] + ring_off;
    }
#endif
    size_t at = 8;
    memcpy(dst, &max_items, 8);
    for (int i = 0; i < count; i++) {
        const PmArg *a = &args[i];
        switch (a->kind) {
            case PM_ARG_INT: memcpy(dst + at, &a->i, 8); at += 8; break;
            case PM_ARG_U32: { uint64_t v = (uint32_t)a->i; memcpy(dst + at, &v, 8); at += 8; break; }
            case PM_ARG_F32: { float v = (float)a->f; uint64_t z = 0; memcpy(&z, &v, 4); memcpy(dst + at, &z, 8); at += 8; break; }
            case PM_ARG_F64: { float hi = (float)a->f, lo = (float)(a->f - (double)hi); memcpy(dst + at, &hi, 4); memcpy(dst + at + 4, &lo, 4); at += 8; break; }
            case PM_ARG_VIEW: {
                PmBufImpl *bi = (PmBufImpl *)a->buffer;
                if (!bi) { note("argument %d: a view without a buffer", i); return PM_ARGUMENT; }
                if (a->offset < 0 || a->count < 0 || (NSUInteger)a->offset > bi->length ||
                    (NSUInteger)a->count > bi->length - (NSUInteger)a->offset) {
                    note("argument %d: a view outside its buffer", i); return PM_ARGUMENT;
                }
                uint64_t address = bi->gpu_addr + (uint64_t)a->offset;
                memcpy(dst + at, &address, 8);
                memcpy(dst + at + 8, &a->count, 8);
                at += 16;
                break;
            }
            default: note("argument %d: unknown kind", i); return PM_ARGUMENT;
        }
    }
    if (b->prof_mode == 1 && b->prof_count < b->prof_cap) {
        if (b->encoder) {
            id<MTLComputeCommandEncoder> old_enc = (__bridge_transfer id<MTLComputeCommandEncoder>)b->encoder;
            [old_enc endEncoding];
            b->encoder = NULL;
        }
        id<MTLCommandBuffer> cmds = (__bridge id<MTLCommandBuffer>)b->commands;
        int idx = b->prof_count++;
        b->prof_recs[idx].name = k->name;
        b->prof_recs[idx].items = max_items;
        b->prof_recs[idx].lanes = lanes;
        id<MTLComputeCommandEncoder> new_enc = prof_open_pass(b, cmds, idx);
        b->encoder = (__bridge_retained void *)new_enc;
    } else if (b->prof_mode == 2 && b->prof_count > 0 && b->prof_count <= b->prof_cap) {
        if (!b->prof_recs[b->prof_count - 1].name) {
            b->prof_recs[b->prof_count - 1].name = k->name;
            b->prof_recs[b->prof_count - 1].items = max_items;
            b->prof_recs[b->prof_count - 1].lanes = lanes;
        }
    }
    id enc = (__bridge id)b->encoder;
    if (b->last_pipeline != kernel) {
        b->fn_set_pso(enc, @selector(setComputePipelineState:), (__bridge id<MTLComputePipelineState>)k->pso);
        b->last_pipeline = kernel;
    }
#if defined(__MAC_26_0)
    if (b->is_m4) {
        if (@available(macOS 26.0, *)) {
            int sl = b->m4_slot;
            m4_arg_off[sl] = ring_off + at;
            m4_fn_set_addr(m4_at[sl], @selector(setAddress:atIndex:), m4_arg_gpu[sl] + (uint64_t)ring_off, 0);
            id<MTL4ComputeCommandEncoder> enc4 = (id<MTL4ComputeCommandEncoder>)enc;
            [enc4 dispatchThreadgroupsWithIndirectBuffer:(ibi->gpu_addr + (uint64_t)indirect_offset)
                                   threadsPerThreadgroup:MTLSizeMake((NSUInteger)lanes, 1, 1)];
        }
        return PM_OK;
    }
#endif
    b->fn_set_bytes(enc, @selector(setBytes:length:atIndex:), dst, at, 0);
    id<MTLBuffer> ibuf = (__bridge id<MTLBuffer>)ibi->buf;
    [(id<MTLComputeCommandEncoder>)enc dispatchThreadgroupsWithIndirectBuffer:ibuf
                                                         indirectBufferOffset:(NSUInteger)indirect_offset
                                                        threadsPerThreadgroup:MTLSizeMake((NSUInteger)lanes, 1, 1)];
    return PM_OK;
}

PmStatus pm_submit_async_after(PmBatch batch, uint64_t wait_ticket, uint64_t *ticket, int *slot_out) {
    @autoreleasepool {
        PmBatchImpl *b = (PmBatchImpl *)batch;
#if defined(__MAC_26_0)
        if (b && b->is_m4) {
            if (@available(macOS 26.0, *)) {
                int sl = b->m4_slot;
                id<MTL4ComputeCommandEncoder> enc4 = (__bridge_transfer id<MTL4ComputeCommandEncoder>)b->encoder;
                free(b);
                [enc4 endEncoding];
                [m4_cb[sl] endCommandBuffer];
                m4_t_submit[sl] = now();
                if (wait_ticket > 0) {
                    [m4_queue waitForEvent:m4_ev value:wait_ticket];
                }
                id<MTL4CommandBuffer> cba = m4_cb[sl];
                [m4_queue commit:&cba count:1];
                uint64_t sig = ++m4_ev_val;
                [m4_queue signalEvent:m4_ev value:sig];
                if (ticket) *ticket = sig;
                if (slot_out) *slot_out = sl;
                return PM_OK;
            }
        }
#endif
        (void)wait_ticket; (void)ticket; (void)slot_out;
        return pm_wait(batch, NULL);
    }
}

PmStatus pm_submit_async(PmBatch batch, uint64_t *ticket, int *slot_out) {
    return pm_submit_async_after(batch, 0, ticket, slot_out);
}

PmStatus pm_wait_ticket(uint64_t ticket, int slot, double *gpu_seconds) {
    @autoreleasepool {
#if defined(__MAC_26_0)
        if (m4_enabled) {
            if (@available(macOS 26.0, *)) {
                int sl = slot & 1;
                if (![m4_ev waitUntilSignaledValue:ticket timeoutMS:60000]) {
                    note("Metal 4 command buffer timed out");
                    return PM_DEVICE;
                }
                double t1 = now();
                double t0 = m4_t_submit[sl];
                if (m4_t_last_done > t0) t0 = m4_t_last_done;
                m4_t_last_done = t1;
                if (gpu_seconds) *gpu_seconds = (t1 > t0) ? (t1 - t0) : 0.0;
                const uint32_t *words = m4_errors[sl].contents;
                if (words[0]) { note("%u Coax checks failed inside the launches (largest source line %u)", words[0], words[1]); return PM_CHECK; }
                return PM_OK;
            }
        }
#endif
        (void)ticket; (void)slot;
        if (gpu_seconds) *gpu_seconds = 0.0;
        return PM_OK;
    }
}

PmStatus pm_wait(PmBatch batch, double *gpu_seconds) {
    @autoreleasepool {
        PmBatchImpl *b = (PmBatchImpl *)batch;
#if defined(__MAC_26_0)
        if (b->is_m4) {
            if (@available(macOS 26.0, *)) {
                uint64_t ticket = 0;
                int sl = 0;
                PmStatus st = pm_submit_async(batch, &ticket, &sl);
                if (st != PM_OK) return st;
                return pm_wait_ticket(ticket, sl, gpu_seconds);
            }
        }
#endif
        id<MTLComputeCommandEncoder> enc = b->encoder ? (__bridge_transfer id<MTLComputeCommandEncoder>)b->encoder : nil;
        id<MTLCommandBuffer> cmds = (__bridge_transfer id<MTLCommandBuffer>)b->commands;
        id<MTLBuffer> errs = (__bridge id<MTLBuffer>)b->errors;
        int prof_mode = b->prof_mode;
        int prof_count = b->prof_count;
        id<MTLCounterSampleBuffer> prof_csb = b->prof_csb ? (__bridge_transfer id<MTLCounterSampleBuffer>)b->prof_csb : nil;
        PmProfRec *prof_recs = b->prof_recs;
        free(b);
        if (enc) [enc endEncoding];
        [cmds commit];
        [cmds waitUntilCompleted];
        double total_gpu = cmds.GPUEndTime - cmds.GPUStartTime;
        if (gpu_seconds) *gpu_seconds = total_gpu;
        if (prof_mode && prof_csb && prof_count > 0) {
            NSData *data = [prof_csb resolveCounterRange:NSMakeRange(0, (NSUInteger)(prof_count * 2))];
            const MTLCounterResultTimestamp *ts = (const MTLCounterResultTimestamp *)data.bytes;
            typedef struct { const char *name; int cnt; double ns; int64_t items_sum; int lanes; } Agg;
            Agg aggs[128];
            int n_agg = 0;
            double sum_ns = 0, gap_ns = 0;
            uint64_t prev_end = 0;
            for (int i = 0; i < prof_count; i++) {
                uint64_t t0 = ts[i * 2].timestamp, t1 = ts[i * 2 + 1].timestamp;
                if (t0 == MTLCounterErrorValue || t1 == MTLCounterErrorValue || t1 < t0) continue;
                double dt = (double)(t1 - t0);
                sum_ns += dt;
                if (prev_end && t0 > prev_end) gap_ns += (double)(t0 - prev_end);
                prev_end = t1;
                const char *nm = prof_recs[i].name ? prof_recs[i].name : "(empty)";
                int k = 0;
                for (; k < n_agg; k++) if (strcmp(aggs[k].name, nm) == 0) break;
                if (k == n_agg && n_agg < 128) { aggs[n_agg].name = nm; aggs[n_agg].cnt = 0; aggs[n_agg].ns = 0; aggs[n_agg].items_sum = 0; aggs[n_agg].lanes = prof_recs[i].lanes; n_agg++; }
                if (k < 128) { aggs[k].cnt++; aggs[k].ns += dt; aggs[k].items_sum += prof_recs[i].items; }
            }
            for (int i = 0; i < n_agg; i++)
                for (int j = i + 1; j < n_agg; j++)
                    if (aggs[j].ns > aggs[i].ns) { Agg tmp = aggs[i]; aggs[i] = aggs[j]; aggs[j] = tmp; }
            fprintf(stderr, "\n[PM_PASS_PROF mode=%d passes=%d total_cmd_gpu=%.3f ms sum_pass=%.3f ms gaps=%.3f ms]\n",
                    prof_mode, prof_count, total_gpu * 1e3, sum_ns * 1e-6, gap_ns * 1e-6);
            for (int i = 0; i < n_agg; i++) {
                double ms = aggs[i].ns * 1e-6;
                fprintf(stderr, "  %-28s cnt=%4d  total=%7.3f ms (%5.1f%%)  avg=%6.2f us  avg_items=%6.1f  lanes=%d\n",
                        aggs[i].name, aggs[i].cnt, ms, 100.0 * aggs[i].ns / (sum_ns > 0 ? sum_ns : 1.0),
                        (aggs[i].ns * 1e-3) / aggs[i].cnt, (double)aggs[i].items_sum / aggs[i].cnt, aggs[i].lanes);
            }
        }
        free(prof_recs);
        if (cmds.status != MTLCommandBufferStatusCompleted) {
            note("command buffer failed: %s", cmds.error ? cmds.error.localizedDescription.UTF8String : "unknown error");
            return PM_DEVICE;
        }
        const uint32_t *words = errs.contents;
        if (words[0]) { note("%u Coax checks failed inside the launches (largest source line %u)", words[0], words[1]); return PM_CHECK; }
        return PM_OK;
    }
}
