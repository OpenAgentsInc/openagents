// One scratch-only Flash gate projection. No model, precision fallback, or tuning loop.
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#import <MetalPerformanceShaders/MetalPerformanceShaders.h>
#import <CommonCrypto/CommonDigest.h>
#include <simd/simd.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

static const NSUInteger guardBytes = 256, N = 1024, M = 12288, K = 4096;
static NSString *phase = @"startup";
static double milliseconds(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec * 1000. + t.tv_nsec / 1.e6; }
static void emit(NSDictionary *row) {
    NSError *error = nil;
    NSData *data = [NSJSONSerialization dataWithJSONObject:row options:NSJSONWritingSortedKeys error:&error];
    if (!data) { fprintf(stderr, "JSON error: %s\n", error.description.UTF8String); exit(3); }
    fwrite(data.bytes, 1, data.length, stdout); fputc('\n', stdout); fflush(stdout);
}
static void require(BOOL condition, NSString *reason) {
    if (!condition) @throw [NSException exceptionWithName:@"ProbeRejected" reason:reason userInfo:nil];
}
static NSData *digest(const void *bytes, NSUInteger count) {
    require(count <= UINT32_MAX, @"hash input exceeds bounded helper");
    unsigned char hash[CC_SHA256_DIGEST_LENGTH]; CC_SHA256(bytes, (CC_LONG)count, hash);
    return [NSData dataWithBytes:hash length:sizeof(hash)];
}
static NSString *hex(NSData *data) {
    NSMutableString *out = [NSMutableString string];
    for (NSUInteger i = 0; i < data.length; ++i) [out appendFormat:@"%02x", ((const uint8_t *)data.bytes)[i]];
    return out;
}
static NSData *readVerified(NSString *path, NSUInteger bytes, NSString *sha) {
    NSError *error = nil;
    NSData *data = [NSData dataWithContentsOfFile:path options:NSDataReadingMappedIfSafe error:&error];
    require(data != nil && data.length == bytes, error.localizedDescription ?: @"wrong asset length");
    require([hex(digest(data.bytes, data.length)) isEqualToString:sha], @"asset SHA-256 mismatch");
    return data;
}
@interface Guarded : NSObject
@property(strong) id<MTLBuffer> buffer;
@property(strong) MPSNDArray *array;
@property(strong) NSData *originalHash;
@property NSUInteger bytes;
@property BOOL immutable;
@end
@implementation Guarded
@end
static void *payload(Guarded *item) { return (uint8_t *)item.buffer.contents + guardBytes; }
static Guarded *makeArray(id<MTLDevice> device, MPSDataType type, NSArray<NSNumber *> *shape,
                          const void *data, NSUInteger bytes, BOOL immutable) {
    require(bytes <= 128 * 1024 * 1024, @"one explicit buffer exceeds 128 MiB");
    Guarded *item = [Guarded new]; item.bytes = bytes; item.immutable = immutable;
    NSUInteger size = ((guardBytes + MAX(bytes, 4096u) + guardBytes + 4095u) / 4096u) * 4096u;
    item.buffer = [device newBufferWithLength:size options:MTLResourceStorageModeShared];
    require(item.buffer != nil, @"buffer allocation failed");
    memset(item.buffer.contents, 0xa5, size);
    if (data) memcpy(payload(item), data, bytes);
    item.originalHash = digest(item.buffer.contents, size);
    MPSNDArrayDescriptor *desc = [MPSNDArrayDescriptor descriptorWithDataType:type shape:shape];
    desc.preferPackedRows = YES;
    item.array = [[MPSNDArray alloc] initWithBuffer:item.buffer offset:guardBytes descriptor:desc];
    require(item.array.dataType == type, @"array storage type changed");
    return item;
}
static void validate(NSArray<Guarded *> *items) {
    for (Guarded *item in items) {
        const uint8_t *data = item.buffer.contents;
        for (NSUInteger i = 0; i < guardBytes; ++i) require(data[i] == 0xa5, @"prefix guard changed");
        for (NSUInteger i = guardBytes + item.bytes; i < item.buffer.length; ++i)
            require(data[i] == 0xa5, @"tail or padding guard changed");
        if (item.immutable) require([digest(data, item.buffer.length) isEqualToData:item.originalHash], @"source bytes changed");
    }
}
static NSUInteger explicitBytes(NSArray<Guarded *> *items) {
    NSUInteger result = 0; for (Guarded *item in items) result += item.buffer.length;
    require(result < 512 * 1024 * 1024, @"explicit GPU storage exceeds 512 MiB");
    return result;
}
static void memoryRow(id<MTLDevice> device, NSArray<Guarded *> *items, NSString *where) {
    NSUInteger arrays = 0; for (Guarded *item in items) arrays += item.array.resourceSize;
    emit(@{@"phase": @"memory", @"where": where, @"explicit_buffer_bytes": @(explicitBytes(items)),
           @"named_ndarray_resource_bytes": @(arrays), @"device_current_allocated_bytes": @(device.currentAllocatedSize),
           @"interpretation": @"point-in-time library allocation accounting; not a private-expansion or peak guarantee"});
}
static void poison(Guarded *out) {
    uint32_t *p = payload(out); for (NSUInteger i = 0; i < out.bytes / 4; ++i) p[i] = 0x7fc12345;
}
static void finish(id<MTLCommandBuffer> command) {
    [command commit]; [command waitUntilCompleted];
    require(command.status == MTLCommandBufferStatusCompleted, command.error.localizedDescription ?: @"GPU command failed");
}
static NSDictionary *compare(Guarded *out, Guarded *reference) {
    const float *a = payload(out), *b = payload(reference);
    double max = 0, sum = 0, squares = 0; NSUInteger over = 0, nonfinite = 0, different = 0;
    NSMutableArray *first = [NSMutableArray array];
    for (NSUInteger i = 0; i < N * M; ++i) {
        if (!isfinite(a[i]) || !isfinite(b[i])) { ++nonfinite; continue; }
        double error = fabs((double)a[i] - (double)b[i]);
        max = fmax(max, error); sum += error; squares += error * error;
        different += memcmp(a + i, b + i, sizeof(float)) != 0;
        if (error > .001) {
            ++over;
            if (first.count < 8) [first addObject:@{@"index": @(i), @"expected": @(b[i]), @"actual": @(a[i]), @"error": @(error)}];
        }
    }
    return @{@"count": @(N * M), @"nonfinite": @(nonfinite), @"different_bits": @(different),
             @"above_fixed_0_001": @(over), @"max_absolute_error": @(max), @"mean_absolute_error": @(sum / (N * M)),
             @"rms_error": @(sqrt(squares / (N * M))), @"first_failures": first};
}
static NSDictionary *oracle(Guarded *out, Guarded *xBuffer, Guarded *wBuffer) {
    const float *actual = payload(out); const _Float16 *x = payload(xBuffer), *w = payload(wBuffer);
    NSMutableArray *samples = [NSMutableArray array]; double max = 0;
    for (NSUInteger s = 0; s < 96; ++s) {
        NSUInteger i = s < 4 ? (s % 2 ? N - 1 : 0) : (s * 97 + 5) % N;
        NSUInteger j = s < 4 ? (s < 2 ? 0 : M - 1) : (s * 997 + 11) % M;
        double sum = 0; for (NSUInteger p = 0; p < K; ++p) sum += (double)x[i * K + p] * (double)w[j * K + p];
        double error = fabs((double)actual[i * M + j] - sum); max = fmax(max, error);
        require(isfinite(sum) && isfinite(actual[i * M + j]), @"nonfinite oracle sample");
        [samples addObject:@{@"row": @(i), @"column": @(j), @"expected_f64": @(sum), @"actual": @(actual[i * M + j]), @"absolute_error": @(error)}];
    }
    return @{@"samples": samples, @"max_absolute_error": @(max), @"sample_count": @96};
}
static NSDictionary *runProjection(id<MTLDevice> device, id<MTLCommandQueue> queue, NSUInteger variant,
    id<MTLComputePipelineState> fixed, id<MTLComputePipelineState> shapes,
    MPSNDArrayMatrixMultiplication *dense, MPSNDArrayQuantizedMatrixMultiplication *lut,
    Guarded *x, Guarded *w, Guarded *wt, Guarded *q, Guarded *table, Guarded *out) {
    @autoreleasepool {
        NSUInteger before = device.currentAllocatedSize; double began = milliseconds();
        id<MTLCommandBuffer> command = [queue commandBuffer];
        if (variant < 2) {
            id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
            [encoder setComputePipelineState:variant ? shapes : fixed];
            [encoder setBuffer:x.buffer offset:guardBytes atIndex:0];
            [encoder setBuffer:w.buffer offset:guardBytes atIndex:1];
            [encoder setBuffer:out.buffer offset:guardBytes atIndex:2];
            vector_uint3 nmk = {(uint32_t)N, (uint32_t)M, (uint32_t)K};
            [encoder setBytes:&nmk length:sizeof(nmk) atIndex:3];
            [encoder dispatchThreadgroups:MTLSizeMake(M / (variant ? 128 : 64), N / 64, 1) threadsPerThreadgroup:MTLSizeMake(128, 1, 1)];
            [encoder endEncoding];
        } else if (variant == 2) {
            [dense encodeToCommandBuffer:command sourceArrays:@[x.array, wt.array] destinationArray:out.array];
        } else {
            [lut encodeToCommandBuffer:command sourceArrays:@[x.array, q.array, table.array] destinationArray:out.array];
        }
        double encoded = milliseconds(); NSUInteger afterEncode = device.currentAllocatedSize;
        finish(command); double ended = milliseconds();
        double gpu = (command.GPUEndTime - command.GPUStartTime) * 1000.;
        require(isfinite(gpu) && gpu > 0, @"GPU command timestamps unavailable");
        return @{@"gpu_command_ms": @(gpu), @"encode_cpu_ms": @(encoded - began), @"encode_wait_wall_ms": @(ended - began),
                 @"device_allocated_before": @(before), @"device_allocated_after_encode": @(afterEncode),
                 @"device_allocated_after_wait": @(device.currentAllocatedSize)};
    }
}

int main(int argc, const char **argv) {
    @autoreleasepool {
        @try {
            require(argc == 3, @"expected source directory and verified asset directory");
            NSString *root = @(argv[1]), *assets = @(argv[2]);
            id<MTLDevice> device = MTLCreateSystemDefaultDevice(); require(device != nil, @"Metal device unavailable");
            id<MTLCommandQueue> queue = [device newCommandQueue]; require(queue != nil, @"command queue unavailable");
            NSError *error = nil;
            id<MTLLibrary> library = [device newLibraryWithFile:[root stringByAppendingPathComponent:@"baseline.metallib"] error:&error];
            require(library != nil, error.localizedDescription ?: @"baseline metallib unavailable");
            id<MTLComputePipelineState> fixed = [device newComputePipelineStateWithFunction:[library newFunctionWithName:@"clef_gemm"] error:&error];
            id<MTLComputePipelineState> shapes = [device newComputePipelineStateWithFunction:[library newFunctionWithName:@"clef_gemm_64x128"] error:&error];
            require(fixed != nil && shapes != nil, error.localizedDescription ?: @"MPP pipeline creation failed");
            for (id<MTLComputePipelineState> pipe in @[fixed, shapes]) require(pipe && pipe.threadExecutionWidth == 32 && pipe.maxTotalThreadsPerThreadgroup >= 128 && pipe.staticThreadgroupMemoryLength <= device.maxThreadgroupMemoryLength, @"MPP pipeline dispatch limits unsupported");
            emit(@{@"phase": @"configuration", @"device": device.name, @"shape_nmk": @[@(N), @(M), @(K)],
                   @"projection": @"layer0 gate, beta0, no residual in this projection", @"input": @"deterministic synthetic half activations",
                   @"precision": @"half activations and exact half weights; explicit float32 output; MPS reduced precision disabled",
                   @"scope": @"one projection; shared machine; no model latency or hidden-expansion guarantee"});
            phase = @"verified_asset_preprocessing"; double prepStart = milliseconds();
            NSMutableArray<Guarded *> *buffers = [NSMutableArray array];
            Guarded *X, *W, *WT, *Q, *L;
            @autoreleasepool {
                NSData *packed = readVerified([assets stringByAppendingPathComponent:@"gate.q4"], M * K / 256 * 144, @"ed85cba6cb0fb593e35f062c149ecd2ae97cc10ba741c7a9c9731225ec0dc7af");
                NSData *halves = readVerified([assets stringByAppendingPathComponent:@"gate.f16"], M * K * 2, @"a2bd893ff0f03262e671775cbadaa946f9b3b501fc2042f146621f36ab2b85ad");
                NSMutableData *qData = [NSMutableData dataWithLength:M * K / 2];
                NSMutableData *lutData = [NSMutableData dataWithLength:(K / 32) * M * 16 * 2];
                NSMutableData *wtData = [NSMutableData dataWithLength:M * K * 2];
                NSMutableData *xData = [NSMutableData dataWithLength:N * K * 2];
                uint8_t *q = qData.mutableBytes; _Float16 *table = lutData.mutableBytes, *wt = wtData.mutableBytes, *x = xData.mutableBytes;
                const uint8_t *raw = packed.bytes; const _Float16 *half = halves.bytes;
                NSUInteger mismatches = 0;
                for (NSUInteger b = 0; b < M * K / 256; ++b) {
                    const uint8_t *block = raw + b * 144; _Float16 dh, mh;
                    memcpy(&dh, block, 2); memcpy(&mh, block + 2, 2);
                    const uint8_t *s = block + 4; NSUInteger row = b / (K / 256), kBlock = b % (K / 256);
                    for (NSUInteger g = 0; g < 8; ++g) {
                        uint8_t sc = g < 4 ? s[g] & 63 : (s[g + 4] & 15) | ((s[g - 4] >> 6) << 4);
                        uint8_t mn = g < 4 ? s[g + 4] & 63 : (s[g + 4] >> 4) | ((s[g] >> 6) << 4);
                        float scale = (float)dh * sc, minimum = (float)mh * mn;
                        NSUInteger group = kBlock * 8 + g;
                        for (NSUInteger v = 0; v < 16; ++v) table[(group * M + row) * 16 + v] = (_Float16)fmaf(scale, (float)v, -minimum);
                        for (NSUInteger t = 0; t < 32; ++t) {
                            uint8_t nibble = block[16 + (g / 2) * 32 + t]; nibble = g % 2 ? nibble >> 4 : nibble & 15;
                            NSUInteger p = kBlock * 256 + g * 32 + t, index = p * M + row;
                            q[index / 2] |= nibble << ((index % 2) * 4);
                            _Float16 decoded = table[(group * M + row) * 16 + nibble];
                            mismatches += memcmp(&decoded, half + row * K + p, 2) != 0;
                        }
                    }
                }
                emit(@{@"phase": @"cpu_all_weight_decode", @"count": @(M * K), @"half_bit_mismatches": @(mismatches)});
                require(mismatches == 0, @"LUT construction changed an independently decoded half weight");
                for (NSUInteger row0 = 0; row0 < M; row0 += 32) for (NSUInteger p0 = 0; p0 < K; p0 += 32)
                    for (NSUInteger r = row0; r < row0 + 32; ++r) for (NSUInteger p = p0; p < p0 + 32; ++p) wt[p * M + r] = half[r * K + p];
                for (NSUInteger i = 0; i < N * K; ++i) x[i] = (_Float16)((float)((int)((i * 17 + 11) % 101) - 50) / 128.f);
                emit(@{@"phase": @"derived_input_hashes", @"q_km_sha256": hex(digest(qData.bytes,qData.length)),
                       @"lut_group_m_16_sha256": hex(digest(lutData.bytes,lutData.length)), @"wt_km_sha256": hex(digest(wtData.bytes,wtData.length)),
                       @"activation_sha256": hex(digest(xData.bytes,xData.length)), @"preprocessing_cpu_ms_before_upload": @(milliseconds()-prepStart)});
                X = makeArray(device,MPSDataTypeFloat16,@[@(N),@(K)],x,xData.length,YES);
                W = makeArray(device,MPSDataTypeFloat16,@[@(M),@(K)],half,halves.length,YES);
                WT = makeArray(device,MPSDataTypeFloat16,@[@(K),@(M)],wt,wtData.length,YES);
                Q = makeArray(device,MPSDataTypeUInt4,@[@(K),@(M)],q,qData.length,YES);
                L = makeArray(device,MPSDataTypeFloat16,@[@(K/32),@(M),@16],table,lutData.length,YES);
                [buffers addObjectsFromArray:@[X,W,WT,Q,L]];
            }
            emit(@{@"phase": @"preprocessing_complete", @"cpu_and_upload_ms": @(milliseconds()-prepStart), @"excluded_from_projection_timings": @YES});
            memoryRow(device,buffers,@"input_upload_complete"); validate(buffers);
            phase = @"gpu_all_weight_decode";
            @autoreleasepool {
                Guarded *D = makeArray(device,MPSDataTypeFloat16,@[@(K),@(M)],NULL,M*K*2,NO);
                MPSNDArrayLUTDequantize *decode = [[MPSNDArrayLUTDequantize alloc] initWithDevice:device]; decode.options = MPSKernelOptionsNone;
                id<MTLCommandBuffer> command = [queue commandBuffer];
                [decode encodeToCommandBuffer:command sourceArrays:@[Q.array,L.array] destinationArray:D.array]; finish(command);
                NSUInteger mismatches = 0; const uint8_t *a = payload(D), *b = payload(WT);
                NSMutableArray *first = [NSMutableArray array];
                for (NSUInteger i = 0; i < M*K; ++i) if (memcmp(a+i*2,b+i*2,2)) {
                    ++mismatches; if (first.count < 8) { uint16_t actual,expected; memcpy(&actual,a+i*2,2); memcpy(&expected,b+i*2,2);
                        [first addObject:@{@"index":@(i),@"actual_bits":@(actual),@"expected_bits":@(expected)}]; }
                }
                validate([buffers arrayByAddingObject:D]); memoryRow(device,[buffers arrayByAddingObject:D],@"exact_decode_complete_before_release");
                emit(@{@"phase":phase,@"count":@(M*K),@"half_bit_mismatches":@(mismatches),@"first_mismatches":first});
                require(mismatches==0,@"full LUT decode differs from independent half weights");
            }
            memoryRow(device,buffers,@"decode_temporary_released");
            Guarded *R=makeArray(device,MPSDataTypeFloat32,@[@(N),@(M)],NULL,N*M*4,NO);
            Guarded *O=makeArray(device,MPSDataTypeFloat32,@[@(N),@(M)],NULL,N*M*4,NO);
            [buffers addObjectsFromArray:@[R,O]];
            MPSNDArrayMatrixMultiplication *dense=[[MPSNDArrayMatrixMultiplication alloc] initWithDevice:device sourceCount:2];
            dense.alpha=1;dense.beta=0;dense.options=MPSKernelOptionsNone;
            MPSNDArrayLUTQuantizationDescriptor *right=[[MPSNDArrayLUTQuantizationDescriptor alloc] initWithDataType:MPSDataTypeUInt4];
            MPSNDArrayQuantizedMatrixMultiplication *lut=[[MPSNDArrayQuantizedMatrixMultiplication alloc] initWithDevice:device leftQuantizationDescriptor:nil rightQuantizationDescriptor:right];
            lut.alpha=1;lut.beta=0;lut.options=MPSKernelOptionsNone;
            NSArray *names=@[@"mpp_fixed64",@"mpp_weight_shapes",@"ndarray_dense_half_f32",@"ndarray_uint4_half_lut_f32"];
            phase=@"reference";poison(R);
            NSDictionary *referenceTiming=runProjection(device,queue,0,fixed,shapes,dense,lut,X,W,WT,Q,L,R);
            R.immutable=YES;R.originalHash=digest(R.buffer.contents,R.buffer.length);
            NSDictionary *refOracle=oracle(R,X,W);
            emit(@{@"phase":phase,@"timing":referenceTiming,@"independent_f64_oracle":refOracle});
            require([refOracle[@"max_absolute_error"] doubleValue]<=.001,@"production baseline failed independent oracle");
            validate(buffers);
            for (NSUInteger variant=0;variant<4;++variant) {
                phase=[@"correctness/" stringByAppendingString:names[variant]];poison(O);
                NSDictionary *timing=runProjection(device,queue,variant,fixed,shapes,dense,lut,X,W,WT,Q,L,O);
                NSDictionary *comparison=compare(O,R),*sample=oracle(O,X,W);
                emit(@{@"phase":phase,@"timing":timing,@"full_output_comparison":comparison,@"independent_f64_oracle":sample});
                require([comparison[@"nonfinite"] unsignedIntegerValue]==0 && [comparison[@"above_fixed_0_001"] unsignedIntegerValue]==0 && [sample[@"max_absolute_error"] doubleValue]<=.001,@"strict projection correctness gate failed");
                validate(buffers);memoryRow(device,buffers,phase);
            }
            for (NSUInteger round=0;round<5;++round) for (NSUInteger position=0;position<4;++position) {
                NSUInteger variant=round%2?3-position:position;
                phase=[@"paired_timing/" stringByAppendingString:names[variant]];poison(O);
                NSDictionary *timing=runProjection(device,queue,variant,fixed,shapes,dense,lut,X,W,WT,Q,L,O);
                NSDictionary *comparison=compare(O,R);
                emit(@{@"phase":phase,@"round":@(round),@"position":@(position),@"variant":names[variant],@"timing":timing,@"full_output_comparison":comparison,
                       @"device_allocated_after_autoreleasepool":@(device.currentAllocatedSize)});
                require([comparison[@"nonfinite"] unsignedIntegerValue]==0 && [comparison[@"above_fixed_0_001"] unsignedIntegerValue]==0,@"timed projection failed unchanged full-output gate");
            }
            validate(buffers);memoryRow(device,buffers,@"all_checks_complete");
            emit(@{@"phase":@"complete",@"status":@"pass",@"model_acceptance":@NO,@"hidden_expansion_excluded":@NO});
            return 0;
        } @catch (NSException *exception) {
            emit(@{@"phase":phase,@"status":@"rejected",@"exception":exception.name,@"reason":exception.reason?:@"unknown",@"fallback_attempted":@NO});
            return 2;
        }
    }
}
