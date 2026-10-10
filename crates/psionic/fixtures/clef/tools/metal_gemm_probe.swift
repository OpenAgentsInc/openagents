// Probe: f16 x f16 -> f32 GEMM on the Metal Performance Primitives matmul2d
// tensor op at Clef-Flash shapes, against MPS. swiftc -O metal_gemm_probe.swift -o probe && ./probe 1024

import Metal
import MetalPerformanceShaders
import Foundation

let src = """
#include <metal_stdlib>
#include <metal_tensor>
#include <MetalPerformancePrimitives/MetalPerformancePrimitives.h>
using namespace metal;
using namespace mpp::tensor_ops;

template <int TM, int TN, int SG>
void gemm_nt_impl(device half *A, device half *B, device float *C, uint3 mnk, uint2 tgid) {
    int M = mnk.x, N = mnk.y, K = mnk.z;
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tA(A, dextents<int32_t, 2>(K, M));
    tensor<device half, dextents<int32_t, 2>, tensor_inline> tB(B, dextents<int32_t, 2>(K, N));
    tensor<device float, dextents<int32_t, 2>, tensor_inline> tC(C, dextents<int32_t, 2>(N, M));
    constexpr auto desc = matmul2d_descriptor(TM, TN, static_cast<int>(dynamic_extent), false, true, false);
    matmul2d<desc, execution_simdgroups<SG>> op;
    auto mA = tA.slice(0, tgid.y * TM);
    auto mB = tB.slice(0, tgid.x * TN);
    auto mC = tC.slice(tgid.x * TN, tgid.y * TM);
    op.run(mA, mB, mC);
}
kernel void gemm_64_32(device half *A [[buffer(0)]], device half *B [[buffer(1)]], device float *C [[buffer(2)]],
                       constant uint3 &mnk [[buffer(3)]], uint2 tgid [[threadgroup_position_in_grid]]) {
    gemm_nt_impl<64, 32, 4>(A, B, C, mnk, tgid);
}
kernel void gemm_128_64(device half *A [[buffer(0)]], device half *B [[buffer(1)]], device float *C [[buffer(2)]],
                        constant uint3 &mnk [[buffer(3)]], uint2 tgid [[threadgroup_position_in_grid]]) {
    gemm_nt_impl<128, 64, 4>(A, B, C, mnk, tgid);
}
kernel void gemm_64_64(device half *A [[buffer(0)]], device half *B [[buffer(1)]], device float *C [[buffer(2)]],
                       constant uint3 &mnk [[buffer(3)]], uint2 tgid [[threadgroup_position_in_grid]]) {
    gemm_nt_impl<64, 64, 4>(A, B, C, mnk, tgid);
}
"""

let device = MTLCreateSystemDefaultDevice()!
print(device.name)
let opts = MTLCompileOptions()
let lib: MTLLibrary
do { lib = try device.makeLibrary(source: src, options: opts) } catch { print("compile error: \(error)"); exit(1) }
let queue = device.makeCommandQueue()!
let args = CommandLine.arguments
let M = args.count > 1 ? Int(args[1])! : 1024
for (N, K) in [(12288, 4096), (4096, 12288), (8192, 4096)] {
    let a = device.makeBuffer(length: M * K * 2, options: .storageModeShared)!
    let b = device.makeBuffer(length: N * K * 2, options: .storageModeShared)!
    let c = device.makeBuffer(length: M * N * 4, options: .storageModeShared)!
    let ap = a.contents().bindMemory(to: Float16.self, capacity: M * K)
    let bp = b.contents().bindMemory(to: Float16.self, capacity: N * K)
    for i in 0..<(M * K) { ap[i] = Float16(Float(i % 7) * 0.01 - 0.03) }
    for i in 0..<(N * K) { bp[i] = Float16(Float(i % 5) * 0.01 - 0.02) }
    var mnk = SIMD3<UInt32>(UInt32(M), UInt32(N), UInt32(K))
    for (name, tm, tn) in [("gemm_64_32", 64, 32), ("gemm_64_64", 64, 64), ("gemm_128_64", 128, 64)] {
        let fn = lib.makeFunction(name: name)!
        let pso = try! device.makeComputePipelineState(function: fn)
        var best = 1e9
        for _ in 0..<5 {
            let cb = queue.makeCommandBuffer()!
            let enc = cb.makeComputeCommandEncoder()!
            enc.setComputePipelineState(pso)
            enc.setBuffer(a, offset: 0, index: 0); enc.setBuffer(b, offset: 0, index: 1); enc.setBuffer(c, offset: 0, index: 2)
            enc.setBytes(&mnk, length: 16, index: 3)
            enc.dispatchThreadgroups(MTLSize(width: (N + tn - 1) / tn, height: (M + tm - 1) / tm, depth: 1),
                                     threadsPerThreadgroup: MTLSize(width: pso.threadExecutionWidth * 4, height: 1, depth: 1))
            enc.endEncoding(); cb.commit(); cb.waitUntilCompleted()
            best = min(best, cb.gpuEndTime - cb.gpuStartTime)
        }
        // check one element
        let cp = c.contents().bindMemory(to: Float.self, capacity: M * N)
        var ref: Float = 0
        let (r, col) = (M / 3, N / 2)
        for k in 0..<K { ref += Float(ap[r * K + k]) * Float(bp[col * K + k]) }
        print(String(format: "M=%d N=%d K=%d %@: %.3f ms %.1f TF  c=%.4f ref=%.4f", M, N, K, name, best * 1e3,
                     2.0 * Double(M * N * K) / best / 1e12, cp[r * N + col], ref))
    }
    // MPS f16 baseline: C = A * B^T
    let da = MPSMatrixDescriptor(rows: M, columns: K, rowBytes: K * 2, dataType: .float16)
    let db = MPSMatrixDescriptor(rows: N, columns: K, rowBytes: K * 2, dataType: .float16)
    let dc = MPSMatrixDescriptor(rows: M, columns: N, rowBytes: N * 4, dataType: .float32)
    let mm = MPSMatrixMultiplication(device: device, transposeLeft: false, transposeRight: true, resultRows: M,
                                     resultColumns: N, interiorColumns: K, alpha: 1, beta: 0)
    var best = 1e9
    for _ in 0..<5 {
        let cb = queue.makeCommandBuffer()!
        mm.encode(commandBuffer: cb, leftMatrix: MPSMatrix(buffer: a, descriptor: da),
                  rightMatrix: MPSMatrix(buffer: b, descriptor: db), resultMatrix: MPSMatrix(buffer: c, descriptor: dc))
        cb.commit(); cb.waitUntilCompleted()
        best = min(best, cb.gpuEndTime - cb.gpuStartTime)
    }
    print(String(format: "M=%d N=%d K=%d MPS: %.3f ms %.1f TF", M, N, K, best * 1e3, 2.0 * Double(M * N * K) / best / 1e12))
}
