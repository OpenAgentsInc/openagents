// Scratch-only public MPSNDArray capability check. No model or timing claim.
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <MetalPerformanceShaders/MetalPerformanceShaders.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static const NSUInteger guardBytes = 256;
static NSString *phase = @"startup";

static void emit(NSDictionary *record) {
    NSError *error = nil;
    NSData *data = [NSJSONSerialization dataWithJSONObject:record options:NSJSONWritingSortedKeys error:&error];
    if (!data) { fprintf(stderr, "JSON error: %s\n", error.description.UTF8String); exit(3); }
    fwrite(data.bytes, 1, data.length, stdout);
    fputc('\n', stdout);
    fflush(stdout);
}

static void require(BOOL condition, NSString *message) {
    if (!condition) @throw [NSException exceptionWithName:@"CapabilityRejected" reason:message userInfo:nil];
}

@interface Guarded : NSObject
@property(strong) id<MTLBuffer> buffer;
@property(strong) MPSNDArray *array;
@property(strong) NSData *original;
@property NSUInteger bytes;
@property BOOL immutable;
@end
@implementation Guarded
@end

static Guarded *makeArray(id<MTLDevice> device, MPSDataType type, NSArray<NSNumber *> *shape,
                          const void *data, NSUInteger bytes, BOOL immutable) {
    Guarded *item = [Guarded new];
    item.bytes = bytes;
    item.immutable = immutable;
    // MPS requires a page-sized backing range even for tiny descriptors. Keep
    // that space after our aligned offset, and guard every unused byte.
    NSUInteger allocation = ((guardBytes + MAX(bytes, 4096u) + guardBytes + 4095u) / 4096u) * 4096u;
    item.buffer = [device newBufferWithLength:allocation options:MTLResourceStorageModeShared];
    require(item.buffer != nil, @"buffer allocation failed");
    memset(item.buffer.contents, 0xa5, allocation);
    if (data) memcpy((uint8_t *)item.buffer.contents + guardBytes, data, bytes);
    item.original = [NSData dataWithBytes:item.buffer.contents length:allocation];
    MPSNDArrayDescriptor *descriptor = [MPSNDArrayDescriptor descriptorWithDataType:type shape:shape];
    item.array = [[MPSNDArray alloc] initWithBuffer:item.buffer offset:guardBytes descriptor:descriptor];
    require(item.array.dataType == type, @"NDArray changed the requested storage type");
    return item;
}

static void validate(NSArray<Guarded *> *buffers) {
    for (Guarded *item in buffers) {
        const uint8_t *now = item.buffer.contents;
        const uint8_t *before = item.original.bytes;
        NSUInteger tail = item.buffer.length - guardBytes - item.bytes;
        require(memcmp(now, before, guardBytes) == 0 &&
                memcmp(now + guardBytes + item.bytes, before + guardBytes + item.bytes, tail) == 0,
                @"a buffer redzone changed");
        if (item.immutable) require(memcmp(now, before, item.buffer.length) == 0,
                                    @"an input buffer changed");
    }
}

static void poison(Guarded *output) {
    // Quiet NaNs expose missing writes. Guards remain unchanged.
    uint32_t *values = (uint32_t *)((uint8_t *)output.buffer.contents + guardBytes);
    for (NSUInteger i = 0; i < output.bytes / 4; ++i) values[i] = 0x7fc12345;
}

static void finish(id<MTLCommandBuffer> command) {
    [command commit];
    [command waitUntilCompleted];
    require(command.status == MTLCommandBufferStatusCompleted,
            command.error.localizedDescription ?: @"command did not complete");
}

static NSArray *numbers(const float *values, NSUInteger count) {
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:count];
    for (NSUInteger i = 0; i < count; ++i) {
        require(isfinite(values[i]), @"nonfinite output");
        [result addObject:@(values[i])];
    }
    return result;
}

static void runCase(id<MTLDevice> device, id<MTLCommandQueue> queue,
                    id<MTLComputePipelineState> add, NSString *name,
                    NSUInteger n, NSUInteger m, NSUInteger k, NSUInteger fixture) {
    require(k % 32 == 0 && m % 2 == 0, @"fixture must have whole groups and nibble pairs");
    const NSUInteger groups = k / 32;
    NSMutableData *xData = [NSMutableData dataWithLength:n * k * sizeof(_Float16)];
    NSMutableData *wData = [NSMutableData dataWithLength:k * m * sizeof(_Float16)];
    NSMutableData *lutData = [NSMutableData dataWithLength:groups * m * 16 * sizeof(_Float16)];
    NSMutableData *qData = [NSMutableData dataWithLength:k * m / 2];
    NSMutableData *resData = [NSMutableData dataWithLength:n * m * sizeof(float)];
    _Float16 *x = xData.mutableBytes, *w = wData.mutableBytes, *lut = lutData.mutableBytes;
    uint8_t *q = qData.mutableBytes;
    float *residual = resData.mutableBytes;
    for (NSUInteger g = 0; g < groups; ++g) for (NSUInteger j = 0; j < m; ++j) {
        for (NSUInteger v = 0; v < 16; ++v) {
            // Same f32 affine FMA followed by one half rounding as Q4_K decode.
            _Float16 d = (_Float16)(.00031f * (float)(g % 5 + 1));
            _Float16 dmin = (_Float16)(.00071f * (float)(j % 3 + 1));
            float scale = (float)d * (float)((g * 7 + j * 3) % 63 + 1);
            float minimum = (float)dmin * (float)((g * 5 + j) % 63 + 1);
            _Float16 weight = (_Float16)fmaf(scale, (float)v, -minimum);
            if (fixture == 1) weight = (_Float16)(v == 0 ? 300.f : v == 1 ? -300.f : 1.f);
            if (fixture == 2) weight = (_Float16)(v == 0 ? 2048.f : v == 1 ? -2048.f : 1.f);
            if (fixture == 3) weight = (_Float16)1.f;
            lut[(g * m + j) * 16 + v] = weight;
        }
    }
    for (NSUInteger p = 0; p < k; ++p) for (NSUInteger j = 0; j < m; ++j) {
        NSUInteger code = (p * 5 + j * 3) % 16;
        if (fixture == 1 || fixture == 2) code = p == 0 ? 0 : p == k - 1 ? 1 : 2;
        w[p * m + j] = lut[((p / 32) * m + j) * 16 + code];
        q[(p * m + j) / 2] |= (uint8_t)(code << (((p * m + j) % 2) * 4));
    }
    for (NSUInteger i = 0; i < n; ++i) for (NSUInteger p = 0; p < k; ++p) {
        float value = (float)((int)((i * 17 + p * 7) % 37) - 18) / 32.f;
        if (fixture == 1) value = p == 0 || p == k - 1 ? 300.f : .125f;
        if (fixture == 2) value = 1.f;
        if (fixture == 3) value = p == 0 ? .5f : .333251953125f;
        x[i * k + p] = (_Float16)value;
    }
    for (NSUInteger i = 0; i < n * m; ++i) residual[i] = (float)((int)(i % 9) - 4) / 131.f;
    Guarded *X = makeArray(device, MPSDataTypeFloat16, @[@(n), @(k)], x, xData.length, YES);
    Guarded *W = makeArray(device, MPSDataTypeFloat16, @[@(k), @(m)], w, wData.length, YES);
    Guarded *Q = makeArray(device, MPSDataTypeUInt4, @[@(k), @(m)], q, qData.length, YES);
    Guarded *L = makeArray(device, MPSDataTypeFloat16, @[@(groups), @(m), @16], lut, lutData.length, YES);
    Guarded *R = makeArray(device, MPSDataTypeFloat32, @[@(n), @(m)], residual, resData.length, YES);
    Guarded *D = makeArray(device, MPSDataTypeFloat16, @[@(k), @(m)], NULL, wData.length, NO);
    Guarded *O = makeArray(device, MPSDataTypeFloat32, @[@(n), @(m)], NULL, resData.length, NO);
    NSArray *buffers = @[X, W, Q, L, R, D, O];
    NSUInteger total = 0;
    for (Guarded *item in buffers) total += item.buffer.length;
    require(total < 2 * 1024 * 1024, @"tiny capability allocation exceeded 2 MiB");

    phase = [name stringByAppendingString:@"/grouped_decode"];
    MPSNDArrayLUTDequantize *decode = [[MPSNDArrayLUTDequantize alloc] initWithDevice:device];
    decode.options = MPSKernelOptionsNone;
    id<MTLCommandBuffer> command = [queue commandBuffer];
    [decode encodeToCommandBuffer:command sourceArrays:@[Q.array, L.array] destinationArray:D.array];
    finish(command);
    validate(buffers);
    require(memcmp((uint8_t *)D.buffer.contents + guardBytes, w, wData.length) == 0,
            @"grouped lookup is not bit-identical to the decoded half weights");
    emit(@{@"phase": phase, @"status": @"pass", @"compared_half_weights": @(k * m)});

    MPSNDArrayMatrixMultiplication *dense = [[MPSNDArrayMatrixMultiplication alloc] initWithDevice:device sourceCount:2];
    dense.alpha = 1; dense.beta = 0; dense.options = MPSKernelOptionsNone;
    MPSNDArrayLUTQuantizationDescriptor *right = [[MPSNDArrayLUTQuantizationDescriptor alloc] initWithDataType:MPSDataTypeUInt4];
    MPSNDArrayQuantizedMatrixMultiplication *quantized = [[MPSNDArrayQuantizedMatrixMultiplication alloc]
        initWithDevice:device leftQuantizationDescriptor:nil rightQuantizationDescriptor:right];
    quantized.alpha = 1; quantized.beta = 0; quantized.options = MPSKernelOptionsNone;
    for (NSUInteger variant = 0; variant < 2; ++variant) {
        for (NSUInteger withResidual = 0; withResidual < 2; ++withResidual) {
            phase = [NSString stringWithFormat:@"%@/%@/residual%lu", name, variant ? @"lut" : @"dense",
                     (unsigned long)withResidual];
            poison(O);
            command = [queue commandBuffer];
            if (variant) [quantized encodeToCommandBuffer:command sourceArrays:@[X.array, Q.array, L.array] destinationArray:O.array];
            else [dense encodeToCommandBuffer:command sourceArrays:@[X.array, W.array] destinationArray:O.array];
            if (withResidual) {
                id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
                [encoder setComputePipelineState:add];
                [encoder setBuffer:O.buffer offset:guardBytes atIndex:0];
                [encoder setBuffer:R.buffer offset:guardBytes atIndex:1];
                uint32_t count = (uint32_t)(n * m);
                [encoder setBytes:&count length:sizeof(count) atIndex:2];
                [encoder dispatchThreads:MTLSizeMake(count, 1, 1) threadsPerThreadgroup:MTLSizeMake(32, 1, 1)];
                [encoder endEncoding];
            }
            finish(command);
            validate(buffers);
            require(O.array.dataType == MPSDataTypeFloat32, @"output must remain float32");
            const float *actual = (float *)((uint8_t *)O.buffer.contents + guardBytes);
            NSMutableArray *expectedValues = [NSMutableArray arrayWithCapacity:n * m];
            double maxError = 0, halfRoundedDistance = 0;
            for (NSUInteger i = 0; i < n; ++i) for (NSUInteger j = 0; j < m; ++j) {
                double sum = 0;
                for (NSUInteger p = 0; p < k; ++p) sum += (double)x[i * k + p] * (double)w[p * m + j];
                float expected = (float)sum;
                if (withResidual) expected += residual[i * m + j];
                require(isfinite(actual[i * m + j]), @"nonfinite float32 output");
                maxError = fmax(maxError, fabs((double)actual[i * m + j] - expected));
                float rounded = (float)(_Float16)(float)sum;
                if (withResidual) rounded += residual[i * m + j];
                halfRoundedDistance = fmax(halfRoundedDistance, fabs((double)rounded - expected));
                [expectedValues addObject:@(expected)];
            }
            emit(@{@"phase": phase, @"status": maxError <= .001 ? @"pass" : @"fail",
                   @"shape_nmk": @[@(n), @(m), @(k)], @"input_storage": @"float16",
                   @"weight_storage": variant ? @"uint4 + grouped exact float16 LUT" : @"float16",
                   @"output_storage": @"float32", @"kernel_options": @0,
                   @"max_absolute_error": @(maxError), @"f16_output_rounding_distance": @(halfRoundedDistance),
                   @"allocated_buffer_bytes": @(total), @"actual": numbers(actual, n * m), @"expected": expectedValues});
            require(maxError <= .001, @"strict f64-oracle absolute-error gate failed");
            if (fixture == 3) require(halfRoundedDistance > .001, @"fixture did not distinguish float16 output");
        }
    }
}

int main(void) {
    @autoreleasepool {
        @try {
            id<MTLDevice> device = MTLCreateSystemDefaultDevice();
            require(device != nil, @"Metal device unavailable");
            id<MTLCommandQueue> queue = [device newCommandQueue];
            require(queue != nil, @"command queue unavailable");
            NSError *error = nil;
            NSString *source = @"#include <metal_stdlib>\nusing namespace metal;\nkernel void add_f32(device float *o [[buffer(0)]], device const float *r [[buffer(1)]], constant uint &n [[buffer(2)]], uint i [[thread_position_in_grid]]) { if(i<n) o[i] += r[i]; }";
            MTLCompileOptions *options = [MTLCompileOptions new];
            options.fastMathEnabled = NO;
            id<MTLLibrary> library = [device newLibraryWithSource:source options:options error:&error];
            require(library != nil, error.localizedDescription ?: @"add shader compilation failed");
            id<MTLComputePipelineState> add = [device newComputePipelineStateWithFunction:[library newFunctionWithName:@"add_f32"] error:&error];
            require(add != nil && add.maxTotalThreadsPerThreadgroup >= 32, @"add pipeline unavailable");
            emit(@{@"phase": @"configuration", @"device": device.name,
                   @"scope": @"tiny synthetic capability only; no weighted, model, or speed acceptance",
                   @"precision_contract": @"half operands, explicit float32 output, reduced precision disabled; f64 oracle and precision-sensitive fixtures",
                   @"accumulator_note": @"internal implementation is opaque; these are behavioral tests, not ISA proof"});
            runCase(device, queue, add, @"grouped_affine", 3, 8, 256, 0);
            runCase(device, queue, add, @"half_product_overflow_cancellation", 3, 8, 256, 1);
            runCase(device, queue, add, @"small_terms_between_large_terms", 3, 8, 256, 2);
            runCase(device, queue, add, @"non_half_representable_output", 3, 8, 256, 3);
            runCase(device, queue, add, @"changed_rows_groups", 17, 6, 512, 0);
            emit(@{@"phase": @"complete", @"status": @"pass", @"weighted_execution_authorized": @NO});
            return 0;
        } @catch (NSException *exception) {
            emit(@{@"phase": phase, @"status": @"rejected", @"exception": exception.name,
                   @"reason": exception.reason ?: @"unknown", @"fallback_attempted": @NO});
            return 2;
        }
    }
}
