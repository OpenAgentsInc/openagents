// Measures the existing Clef GEMM at increasing command-buffer lengths.
// Run under a quiet lease. This synthetic probe does not measure inference.
// swiftc -O metal_sustained_gemm.swift -o metal-sustained-gemm
// ./metal-sustained-gemm path/to/clef_prefill.metal > gemm.jsonl
import Foundation
import Metal

let source = try String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
let device = MTLCreateSystemDefaultDevice()!
let library = try device.makeLibrary(source: source, options: MTLCompileOptions())
let pipeline = try device.makeComputePipelineState(function: library.makeFunction(name: "clef_gemm")!)
let queue = device.makeCommandQueue()!
let (n, m, k) = (2048, 12288, 4096)
let input = device.makeBuffer(length: n * k * 2, options: .storageModeShared)!
let output = device.makeBuffer(length: n * m * 4, options: .storageModeShared)!
input.contents().bindMemory(to: Float16.self, capacity: n * k).initialize(repeating: 0.125, count: n * k)
var weights: [MTLBuffer] = []
for _ in 0..<48 {
    let buffer = device.makeBuffer(length: m * k * 2, options: .storageModeShared)!
    buffer.contents().bindMemory(to: Float16.self, capacity: m * k).initialize(repeating: 0.03125, count: m * k)
    weights.append(buffer)
}

func pass(count: Int, distinct: Bool) throws -> (Double, Double) {
    let started = ProcessInfo.processInfo.systemUptime
    let command = queue.makeCommandBuffer()!
    let encoder = command.makeComputeCommandEncoder()!
    encoder.setComputePipelineState(pipeline)
    var shape = SIMD4<UInt32>(UInt32(n), UInt32(m), UInt32(k), 0)
    for index in 0..<count {
        encoder.setBuffer(input, offset: 0, index: 0)
        encoder.setBuffer(distinct ? weights[index] : weights[0], offset: 0, index: 1)
        encoder.setBuffer(output, offset: 0, index: 2)
        encoder.setBytes(&shape, length: 16, index: 3)
        encoder.dispatchThreadgroups(
            MTLSize(width: m / 64, height: n / 64, depth: 1),
            threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1)
        )
    }
    encoder.endEncoding()
    command.commit()
    command.waitUntilCompleted()
    if let error = command.error { throw error }
    precondition(command.status == .completed)
    let gpu = command.gpuEndTime - command.gpuStartTime
    precondition(gpu > 0)
    // All inputs are exact powers of two; every output should equal 16.
    let values = output.contents().bindMemory(to: Float.self, capacity: n * m)
    precondition(values[0] == 16 && values[n * m - 1] == 16)
    return (gpu, ProcessInfo.processInfo.systemUptime - started)
}

// Warm up the kernel, then report every sample rather than selecting minima.
let warmUntil = ProcessInfo.processInfo.systemUptime + 2
while ProcessInfo.processInfo.systemUptime < warmUntil {
    _ = try pass(count: 4, distinct: true)
}
for round in 0..<5 {
    let counts = round % 2 == 0 ? [1, 4, 16, 48] : [48, 16, 4, 1]
    for count in counts {
        let orders = round % 2 == 0 ? [false, true] : [true, false]
        for distinct in orders {
            let (gpu, wall) = try pass(count: count, distinct: distinct)
            let flops = 2.0 * Double(n) * Double(m) * Double(k) * Double(count)
            let row: [String: Any] = [
                "round": round, "count": count, "distinct_weights": distinct,
                "n": n, "m": m, "k": k, "gpu_seconds": gpu, "wall_seconds": wall,
                "tflops": flops / gpu / 1e12, "device": device.name,
                "correct_sampled_outputs": true
            ]
            let data = try JSONSerialization.data(withJSONObject: row, options: .sortedKeys)
            print(String(data: data, encoding: .utf8)!)
            fflush(stdout)
        }
    }
}
