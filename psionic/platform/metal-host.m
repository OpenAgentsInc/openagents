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
                options.mathMode = MTLMathModeFast;
                options.mathFloatingPointFunctions = MTLMathFloatingPointFunctionsFast;
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
} PmKernelImpl;

typedef void (*ImpSetPSO)(id, SEL, id<MTLComputePipelineState>);
typedef void (*ImpSetBytes)(id, SEL, const void *, NSUInteger, NSUInteger);
typedef void (*ImpDispatchTG)(id, SEL, MTLSize, MTLSize);
typedef void (*ImpBarrier)(id, SEL, MTLBarrierScope);

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
    free(bi);
    return PM_OK;
}

void *pm_contents(PmBuffer buffer) { return buffer ? ((PmBufImpl *)buffer)->contents : NULL; }

static PmStatus begin_impl(PmBatch *batch, MTLDispatchType dtype) {
    @autoreleasepool {
        if (!device) return PM_ABSENT;
        PmBatchImpl *b = (PmBatchImpl *)calloc(1, sizeof *b);
        if (!b) return PM_MEMORY;
        id<MTLCommandBuffer> cmds = [queue commandBufferWithUnretainedReferences];
        id<MTLComputeCommandEncoder> enc = [cmds computeCommandEncoderWithDispatchType:dtype];
        if (!shared_errors) {
            shared_errors = [device newBufferWithLength:8 options:MTLResourceStorageModeShared];
        }
        memset(shared_errors.contents, 0, 8);
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
        b->commands = (__bridge_retained void *)cmds;
        b->encoder = (__bridge_retained void *)enc;
        b->errors = (__bridge void *)shared_errors;
        *batch = b;
        return PM_OK;
    }
}

PmStatus pm_begin(PmBatch *batch) { return begin_impl(batch, MTLDispatchTypeSerial); }
PmStatus pm_begin_concurrent(PmBatch *batch) { return begin_impl(batch, MTLDispatchTypeConcurrent); }
void pm_barrier(PmBatch batch) {
    PmBatchImpl *b = (PmBatchImpl *)batch;
    b->fn_barrier((__bridge id)b->encoder, @selector(memoryBarrierWithScope:), MTLBarrierScopeBuffers);
}

PmStatus pm_launch(PmBatch batch, PmKernel kernel, int64_t items, int lanes, int block, const PmArg *args, int count) {
    PmBatchImpl *b = (PmBatchImpl *)batch;
    PmKernelImpl *k = (PmKernelImpl *)kernel;
    if (items < 0 || (lanes != 1 && (lanes % 32 != 0 || lanes > 1024)) || count < 0 || count > 32) { note("malformed launch"); return PM_ARGUMENT; }
    if (items == 0) return PM_OK;
    uint8_t block_bytes[8 + 32 * 16];
    size_t at = 8;
    memcpy(block_bytes, &items, 8);
    for (int i = 0; i < count; i++) {
        const PmArg *a = &args[i];
        switch (a->kind) {
            case PM_ARG_INT:
                memcpy(block_bytes + at, &a->i, 8);
                at += 8;
                break;
            case PM_ARG_U32: {
                uint64_t v = (uint32_t)a->i;
                memcpy(block_bytes + at, &v, 8);
                at += 8;
                break;
            }
            case PM_ARG_F32: {
                float v = (float)a->f;
                uint64_t z = 0;
                memcpy(&z, &v, 4);
                memcpy(block_bytes + at, &z, 8);
                at += 8;
                break;
            }
            case PM_ARG_F64: {
                float hi = (float)a->f, lo = (float)(a->f - (double)hi);
                memcpy(block_bytes + at, &hi, 4);
                memcpy(block_bytes + at + 4, &lo, 4);
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
                memcpy(block_bytes + at, &address, 8);
                memcpy(block_bytes + at + 8, &a->count, 8);
                at += 16;
                break;
            }
            default: note("argument %d: unknown kind", i); return PM_ARGUMENT;
        }
    }
    id enc = (__bridge id)b->encoder;
    if (b->last_pipeline != kernel) {
        b->fn_set_pso(enc, @selector(setComputePipelineState:), (__bridge id<MTLComputePipelineState>)k->pso);
        b->last_pipeline = kernel;
    }
    b->fn_set_bytes(enc, @selector(setBytes:length:atIndex:), block_bytes, at, 0);
    if (lanes > 32) {
        b->fn_dispatch_tg(enc, @selector(dispatchThreadgroups:threadsPerThreadgroup:),
                          MTLSizeMake((NSUInteger)items, 1, 1), MTLSizeMake((NSUInteger)lanes, 1, 1));
        return PM_OK;
    }
    NSUInteger threads = (NSUInteger)items * (NSUInteger)lanes;
    NSUInteger limit = k->max_threads;
    NSUInteger group = block > 0 ? (NSUInteger)block : 128;
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
    if (!ibi || max_items < 0 || lanes <= 32 || lanes % 32 != 0 || lanes > 1024 || count < 0 || count > 16) {
        note("malformed indirect launch");
        return PM_ARGUMENT;
    }
    if (max_items == 0) return PM_OK;
    id<MTLComputeCommandEncoder> enc = (__bridge id<MTLComputeCommandEncoder>)b->encoder;
    id<MTLBuffer> ibuf = (__bridge id<MTLBuffer>)ibi->buf;
    uint8_t block_bytes[8 + 16 * 16];
    size_t at = 8;
    memcpy(block_bytes, &max_items, 8);
    for (int i = 0; i < count; i++) {
        const PmArg *a = &args[i];
        switch (a->kind) {
            case PM_ARG_INT: memcpy(block_bytes + at, &a->i, 8); at += 8; break;
            case PM_ARG_U32: { uint64_t v = (uint32_t)a->i; memcpy(block_bytes + at, &v, 8); at += 8; break; }
            case PM_ARG_F32: { float v = (float)a->f; uint64_t z = 0; memcpy(&z, &v, 4); memcpy(block_bytes + at, &z, 8); at += 8; break; }
            case PM_ARG_F64: { float hi = (float)a->f, lo = (float)(a->f - (double)hi); memcpy(block_bytes + at, &hi, 4); memcpy(block_bytes + at + 4, &lo, 4); at += 8; break; }
            case PM_ARG_VIEW: {
                PmBufImpl *bi = (PmBufImpl *)a->buffer;
                if (!bi) { note("argument %d: a view without a buffer", i); return PM_ARGUMENT; }
                if (a->offset < 0 || a->count < 0 || (NSUInteger)a->offset > bi->length ||
                    (NSUInteger)a->count > bi->length - (NSUInteger)a->offset) {
                    note("argument %d: a view outside its buffer", i); return PM_ARGUMENT;
                }
                uint64_t address = bi->gpu_addr + (uint64_t)a->offset;
                memcpy(block_bytes + at, &address, 8);
                memcpy(block_bytes + at + 8, &a->count, 8);
                at += 16;
                break;
            }
            default: note("argument %d: unknown kind", i); return PM_ARGUMENT;
        }
    }
    if (b->last_pipeline != kernel) {
        b->fn_set_pso(enc, @selector(setComputePipelineState:), (__bridge id<MTLComputePipelineState>)k->pso);
        b->last_pipeline = kernel;
    }
    b->fn_set_bytes(enc, @selector(setBytes:length:atIndex:), block_bytes, at, 0);
    [enc dispatchThreadgroupsWithIndirectBuffer:ibuf
                           indirectBufferOffset:(NSUInteger)indirect_offset
                          threadsPerThreadgroup:MTLSizeMake((NSUInteger)lanes, 1, 1)];
    return PM_OK;
}

PmStatus pm_wait(PmBatch batch, double *gpu_seconds) {
    @autoreleasepool {
        PmBatchImpl *b = (PmBatchImpl *)batch;
        id<MTLComputeCommandEncoder> enc = (__bridge_transfer id<MTLComputeCommandEncoder>)b->encoder;
        id<MTLCommandBuffer> cmds = (__bridge_transfer id<MTLCommandBuffer>)b->commands;
        id<MTLBuffer> errs = (__bridge id<MTLBuffer>)b->errors;
        free(b);
        [enc endEncoding];
        [cmds commit];
        [cmds waitUntilCompleted];
        if (gpu_seconds) *gpu_seconds = cmds.GPUEndTime - cmds.GPUStartTime;
        if (cmds.status != MTLCommandBufferStatusCompleted) {
            note("command buffer failed: %s", cmds.error ? cmds.error.localizedDescription.UTF8String : "unknown error");
            return PM_DEVICE;
        }
        const uint32_t *words = errs.contents;
        if (words[0]) { note("%u Coax checks failed inside the launches (largest source line %u)", words[0], words[1]); return PM_CHECK; }
        return PM_OK;
    }
}
