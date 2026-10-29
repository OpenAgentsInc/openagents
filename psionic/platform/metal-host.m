/* Device effects for Coax-generated Metal compute kernels (see metal-host.h).
   Build with -fobjc-arc -framework Metal -framework Foundation. */
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
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

PmStatus pm_compile(const char *source, size_t length, const char *name, PmModule *module, double *seconds) {
    @autoreleasepool {
        if (!device) return PM_ABSENT;
        double t0 = now();
        NSString *text = [[NSString alloc] initWithBytes:source length:length encoding:NSUTF8StringEncoding];
        if (!text) { note("%s: not UTF-8", name); return PM_ARGUMENT; }
        MTLCompileOptions *options = [MTLCompileOptions new];
        /* IEEE arithmetic, one rounding per operation, precise math functions. */
        options.mathMode = MTLMathModeSafe; /* macOS 15 or later */
        options.mathFloatingPointFunctions = MTLMathFloatingPointFunctionsPrecise;
        options.languageVersion = MTLLanguageVersion3_1;
        NSError *error = nil;
        id<MTLLibrary> library = [device newLibraryWithSource:text options:options error:&error];
        if (!library) { note("%s: %s", name, error.localizedDescription.UTF8String); return PM_COMPILE; }
        *module = (__bridge_retained void *)library;
        if (seconds) *seconds = now() - t0;
        return PM_OK;
    }
}

PmStatus pm_kernel(PmModule module, const char *name, PmKernel *kernel) {
    @autoreleasepool {
        id<MTLLibrary> library = (__bridge id<MTLLibrary>)module;
        id<MTLFunction> function = [library newFunctionWithName:[NSString stringWithUTF8String:name]];
        if (!function) { note("no kernel %s", name); return PM_ARGUMENT; }
        NSError *error = nil;
        id<MTLComputePipelineState> pipeline = [device newComputePipelineStateWithFunction:function error:&error];
        if (!pipeline) { note("%s: %s", name, error.localizedDescription.UTF8String); return PM_COMPILE; }
        if (getenv("PM_VERBOSE")) fprintf(stderr, "kernel %s: %lu threads per group, simd width %lu\n", name, (unsigned long)pipeline.maxTotalThreadsPerThreadgroup, (unsigned long)pipeline.threadExecutionWidth);
        *kernel = (__bridge_retained void *)pipeline;
        return PM_OK;
    }
}

PmStatus pm_alloc(size_t bytes, PmBuffer *buffer) {
    @autoreleasepool {
        if (!device) return PM_ABSENT;
        if (bytes == 0) bytes = 16;
        id<MTLBuffer> b = [device newBufferWithLength:bytes options:MTLResourceStorageModeShared | MTLResourceHazardTrackingModeTracked];
        if (!b) { note("cannot allocate %zu bytes", bytes); return PM_MEMORY; }
        memset(b.contents, 0, bytes);
        [live addObject:b];
        *buffer = (__bridge_retained void *)b;
        return PM_OK;
    }
}

PmStatus pm_free(PmBuffer buffer) {
    if (!buffer) return PM_OK;
    id<MTLBuffer> b = (__bridge_transfer id<MTLBuffer>)buffer;
    [live removeObjectIdenticalTo:b];
    return PM_OK;
}

void *pm_contents(PmBuffer buffer) { return buffer ? ((__bridge id<MTLBuffer>)buffer).contents : NULL; }

@interface PmBatchObject : NSObject
@property(strong) id<MTLCommandBuffer> commands;
@property(strong) id<MTLComputeCommandEncoder> encoder;
@property(strong) id<MTLBuffer> errors;
@end
@implementation PmBatchObject
@end

PmStatus pm_begin(PmBatch *batch) {
    @autoreleasepool {
        if (!device) return PM_ABSENT;
        PmBatchObject *b = [PmBatchObject new];
        b.commands = [queue commandBufferWithUnretainedReferences];
        b.encoder = [b.commands computeCommandEncoderWithDispatchType:MTLDispatchTypeSerial];
        b.errors = [device newBufferWithLength:8 options:MTLResourceStorageModeShared];
        memset(b.errors.contents, 0, 8);
        NSUInteger n = live.count;
        if (n) {
            __unsafe_unretained id<MTLResource> resources[n];
            for (NSUInteger i = 0; i < n; i++) resources[i] = live[i];
            [b.encoder useResources:resources count:n usage:MTLResourceUsageRead | MTLResourceUsageWrite];
        }
        [b.encoder setBuffer:b.errors offset:0 atIndex:1];
        *batch = (__bridge_retained void *)b;
        return PM_OK;
    }
}

PmStatus pm_launch(PmBatch batch, PmKernel kernel, int64_t items, int lanes, int block, const PmArg *args, int count) {
    PmBatchObject *b = (__bridge PmBatchObject *)batch;
    id<MTLComputePipelineState> pipeline = (__bridge id<MTLComputePipelineState>)kernel;
    if (items < 0 || (lanes != 1 && lanes != 32) || count < 0 || count > 16) { note("malformed launch"); return PM_ARGUMENT; }
    if (items == 0) return PM_OK;
    uint8_t block_bytes[8 + 16 * 16];
    size_t at = 0;
    memcpy(block_bytes, &items, 8);
    at = 8;
    for (int k = 0; k < count; k++) {
        const PmArg *a = &args[k];
        uint8_t slot[16] = {0};
        size_t size = 8;
        switch (a->kind) {
            case PM_ARG_INT: memcpy(slot, &a->i, 8); break;
            case PM_ARG_U32: { uint32_t v = (uint32_t)a->i; memcpy(slot, &v, 4); break; }
            case PM_ARG_F32: { float v = (float)a->f; memcpy(slot, &v, 4); break; }
            case PM_ARG_F64: { float hi = (float)a->f, lo = (float)(a->f - (double)hi); memcpy(slot, &hi, 4); memcpy(slot + 4, &lo, 4); break; }
            case PM_ARG_VIEW: {
                if (!a->buffer) { note("argument %d: a view without a buffer", k); return PM_ARGUMENT; }
                id<MTLBuffer> buffer = (__bridge id<MTLBuffer>)a->buffer;
                if (a->offset < 0 || a->count < 0 || (NSUInteger)a->offset > buffer.length) { note("argument %d: a view outside its buffer", k); return PM_ARGUMENT; }
                uint64_t address = buffer.gpuAddress + (uint64_t)a->offset;
                memcpy(slot, &address, 8);
                memcpy(slot + 8, &a->count, 8);
                size = 16;
                break;
            }
            default: note("argument %d: unknown kind", k); return PM_ARGUMENT;
        }
        memcpy(block_bytes + at, slot, size);
        at += size;
    }
    NSUInteger threads = (NSUInteger)items * (NSUInteger)lanes;
    NSUInteger group = block > 0 ? (NSUInteger)block : 128;
    NSUInteger limit = pipeline.maxTotalThreadsPerThreadgroup;
    if (group > limit) group = limit;
    group = group / 32 * 32;
    if (group == 0) { note("pipeline allows fewer than 32 threads per group"); return PM_ARGUMENT; }
    [b.encoder setComputePipelineState:pipeline];
    [b.encoder setBytes:block_bytes length:at atIndex:0];
    [b.encoder dispatchThreads:MTLSizeMake(threads, 1, 1) threadsPerThreadgroup:MTLSizeMake(group, 1, 1)];
    return PM_OK;
}

PmStatus pm_wait(PmBatch batch, double *gpu_seconds) {
    @autoreleasepool {
        PmBatchObject *b = (__bridge_transfer PmBatchObject *)batch;
        [b.encoder endEncoding];
        [b.commands commit];
        [b.commands waitUntilCompleted];
        if (gpu_seconds) *gpu_seconds = b.commands.GPUEndTime - b.commands.GPUStartTime;
        if (b.commands.status != MTLCommandBufferStatusCompleted) {
            note("command buffer failed: %s", b.commands.error ? b.commands.error.localizedDescription.UTF8String : "unknown error");
            return PM_DEVICE;
        }
        const uint32_t *words = b.errors.contents;
        if (words[0]) { note("%u Coax checks failed inside the launches (largest source line %u)", words[0], words[1]); return PM_CHECK; }
        return PM_OK;
    }
}
