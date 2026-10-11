// One fixed dispatch-order experiment; all weights remain in their original layout.
import Foundation
import Metal
import CryptoKit
import Darwin

struct Failure: Error, CustomStringConvertible {
    let description: String
    init(_ value: String) { description = value }
}
func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw Failure(message) }
}
func emit(_ value: [String: Any]) throws {
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]))
    FileHandle.standardOutput.write(Data([10]))
}
func hash(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
func loadAverage() -> [Double] {
    var values = [Double](repeating: 0, count: 3)
    let count = values.withUnsafeMutableBufferPointer { getloadavg($0.baseAddress!, 3) }
    return count == 3 ? values : []
}
let arguments = CommandLine.arguments
try require(arguments.count == 4 && ["check", "bench"].contains(arguments[2]), "Usage: probe DIRECTORY check|bench ASSETS")
let root = URL(fileURLWithPath: arguments[1]), assets = URL(fileURLWithPath: arguments[3])
let benchmark = arguments[2] == "bench"
let started = ProcessInfo.processInfo.systemUptime
let dataLimit = 1024 * 1024 * 1024
let driverLimit = 1536 * 1024 * 1024
let pad = 256
let poison = Float(bitPattern: 0x7fc00555)
var liveBytes = 0, peakBytes = 0, commands = 0, dispatches = 0
func timeCheck() throws {
    try require(ProcessInfo.processInfo.systemUptime - started < 55, "internal 55-second deadline reached")
}
final class Buffer {
    let metal: MTLBuffer
    let bytes: Int
    var pointer: UnsafeMutableRawPointer { metal.contents().advanced(by: pad) }
    init(_ device: MTLDevice, _ bytes: Int) throws {
        try require(bytes > 0 && bytes <= dataLimit - 2 * pad, "invalid buffer size")
        let total = bytes + 2 * pad
        try require(liveBytes <= dataLimit - total, "explicit live buffers exceed 1 GiB")
        guard let allocation = device.makeBuffer(length: total, options: .storageModeShared) else { throw Failure("buffer allocation failed") }
        self.metal = allocation; self.bytes = bytes
        liveBytes += total; peakBytes = max(peakBytes, liveBytes)
        memset(allocation.contents(), 0xa5, total)
        try require(device.currentAllocatedSize <= driverLimit, "Metal allocations exceed 1.5 GiB including private expansion")
    }
    deinit { liveBytes -= bytes + 2 * pad }
    func digest() -> String { hash(Data(bytesNoCopy: metal.contents(), count: bytes + 2 * pad, deallocator: .none)) }
    func guards() throws {
        let p = metal.contents().assumingMemoryBound(to: UInt8.self)
        for i in 0..<pad { try require(p[i] == 0xa5 && p[pad + bytes + i] == 0xa5, "buffer redzone changed") }
    }
    func poisonFloat() { pointer.assumingMemoryBound(to: Float.self).initialize(repeating: poison, count: bytes / 4) }
    func poisonHalf() { pointer.assumingMemoryBound(to: Float16.self).initialize(repeating: Float16(bitPattern: 0x7e55), count: bytes / 2) }
}
func fillHalf(_ buffer: Buffer, width: Int, seed: Int, scale: Float) {
    let p = buffer.pointer.assumingMemoryBound(to: Float16.self)
    for i in 0..<(buffer.bytes / 2) { p[i] = Float16(Float((i * 31 + i / width * 17 + seed) % 251 - 125) / 125 * scale) }
}
func residual(_ buffer: Buffer) {
    let p = buffer.pointer.assumingMemoryBound(to: Float.self)
    for i in 0..<(buffer.bytes / 4) { p[i] = Float((i * 13 + 17) % 101 - 50) / 37 }
}
func exact(_ a: Buffer, _ b: Buffer, half: Bool = false) throws -> Int {
    try require(a.bytes == b.bytes, "comparison length differs")
    try a.guards(); try b.guards()
    if half {
        let p = a.pointer.assumingMemoryBound(to: Float16.self), q = b.pointer.assumingMemoryBound(to: Float16.self)
        for i in 0..<(a.bytes / 2) { try require(p[i].isFinite && q[i].isFinite, "nonfinite or unwritten half output") }
    } else {
        let p = a.pointer.assumingMemoryBound(to: Float.self), q = b.pointer.assumingMemoryBound(to: Float.self)
        for i in 0..<(a.bytes / 4) { try require(p[i].isFinite && q[i].isFinite, "nonfinite or unwritten float output") }
    }
    if memcmp(a.pointer, b.pointer, a.bytes) != 0 {
        var differences = 0, maximum = 0.0, first = -1
        if half {
            let p = a.pointer.assumingMemoryBound(to: Float16.self), q = b.pointer.assumingMemoryBound(to: Float16.self)
            for i in 0..<(a.bytes / 2) { if p[i].bitPattern != q[i].bitPattern { differences += 1; if first < 0 { first = i }; maximum = max(maximum, abs(Double(p[i]) - Double(q[i]))) } }
        } else {
            let p = a.pointer.assumingMemoryBound(to: Float.self), q = b.pointer.assumingMemoryBound(to: Float.self)
            for i in 0..<(a.bytes / 4) { if p[i].bitPattern != q[i].bitPattern { differences += 1; if first < 0 { first = i }; maximum = max(maximum, abs(Double(p[i]) - Double(q[i]))) } }
        }
        try emit(["kind": "bitwise_failure", "half": half, "values": a.bytes / (half ? 2 : 4), "bit_differences": differences, "first_index": first, "max_abs": maximum])
        throw Failure("full output is not bitwise equal")
    }
    return a.bytes / (half ? 2 : 4)
}
// This exact shape table mirrors the retained ClefGemmPlan::new WeightShapes match.
func tile(_ m: Int, _ k: Int) -> Int {
    if m == 32 && k == 4096 { return 32 }
    if (k == 4096 && [4096, 8192, 12288].contains(m)) || (m == 4096 && k == 12288) { return 128 }
    if (k == 5120 && [6144, 10240, 12288, 17408].contains(m)) || (m == 5120 && [6144, 17408].contains(k)) { return 128 }
    return 64
}
func kernel(_ swizzle: Bool, _ m: Int, _ k: Int, _ accumulate: Bool) -> String {
    let t = tile(m, k)
    let stem = swizzle ? "swizzle_\(t)" : (t == 64 ? "clef_gemm" : "clef_gemm_64x\(t)")
    return stem + (accumulate ? "_accumulate" : "")
}
struct Timing { let gpu, wall: Double }
final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    var pipelines = [String: MTLComputePipelineState]()
    init() throws {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { throw Failure("Metal unavailable") }
        self.device = device; self.queue = queue
        let library = try device.makeLibrary(URL: root.appendingPathComponent("kernels.metallib"))
        for t in [32, 64, 128] {
            for swizzle in [false, true] { for acc in [false, true] {
                let stem = swizzle ? "swizzle_\(t)" : (t == 64 ? "clef_gemm" : "clef_gemm_64x\(t)")
                let name = stem + (acc ? "_accumulate" : "")
                guard let function = library.makeFunction(name: name) else { throw Failure("kernel missing: \(name)") }
                let pipeline = try device.makeComputePipelineState(function: function)
                try require(pipeline.threadExecutionWidth == 32 && pipeline.maxTotalThreadsPerThreadgroup >= 128, "pipeline thread count unsupported")
                try require(pipeline.staticThreadgroupMemoryLength <= device.maxThreadgroupMemoryLength, "pipeline shared memory unsupported")
                pipelines[name] = pipeline
            }}
        }
        guard let silu = library.makeFunction(name: "clef_silu_mul_to_f16") else { throw Failure("SiLU kernel missing") }
        let pipeline = try device.makeComputePipelineState(function: silu)
        try require(pipeline.maxTotalThreadsPerThreadgroup >= 128, "SiLU threads unsupported")
        pipelines["silu"] = pipeline
        try emit(["kind": "device", "name": device.name, "current_allocated_bytes": device.currentAllocatedSize, "recommended_working_set_bytes": device.recommendedMaxWorkingSetSize, "explicit_buffer_limit_bytes": dataLimit, "driver_allocation_limit_bytes": driverLimit, "load_average": loadAverage()])
    }
    func memory(_ stage: String) throws {
        try require(device.currentAllocatedSize <= driverLimit, "Metal allocations exceed 1.5 GiB including private expansion")
        try emit(["kind": "allocation", "stage": stage, "current_allocated_bytes": device.currentAllocatedSize, "explicit_live_bytes": liveBytes, "explicit_peak_bytes": peakBytes])
    }
    func gemm(_ e: MTLComputeCommandEncoder, _ swizzle: Bool, x: Buffer, w: Buffer, y: Buffer, n: Int, m: Int, k: Int, acc: Bool = false) {
        e.memoryBarrier(scope: .buffers)
        e.setComputePipelineState(pipelines[kernel(swizzle, m, k, acc)]!)
        for (i, b) in [x, w, y].enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
        var args = SIMD4<UInt32>(UInt32(n), UInt32(m), UInt32(k), 0)
        e.setBytes(&args, length: 16, index: 3)
        let columns = (m + tile(m, k) - 1) / tile(m, k), rows = (n + 63) / 64
        let grid = MTLSize(width: columns * (swizzle ? 4 : 1), height: swizzle ? (rows + 3) / 4 : rows, depth: 1)
        e.dispatchThreadgroups(grid, threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
        dispatches += 1
    }
    func submit(_ f: (MTLComputeCommandEncoder) -> Void) throws -> Timing {
        try timeCheck()
        return try autoreleasepool {
            let began = ProcessInfo.processInfo.systemUptime
            guard let cb = queue.makeCommandBuffer(), let e = cb.makeComputeCommandEncoder(dispatchType: .concurrent) else { throw Failure("command allocation failed") }
            f(e); e.endEncoding(); cb.commit(); cb.waitUntilCompleted(); commands += 1
            try require(cb.status == .completed, "GPU command failed: \(String(describing: cb.error))")
            try require(device.currentAllocatedSize <= driverLimit, "private Metal allocation exceeded limit")
            let gpu = cb.gpuEndTime - cb.gpuStartTime
            try require(gpu.isFinite && gpu > 0, "invalid GPU timestamp")
            return Timing(gpu: gpu, wall: ProcessInfo.processInfo.systemUptime - began)
        }
    }
    func ffn(_ swizzle: Bool, _ weights: Weights, _ x: Buffer, _ state: State, n: Int, acc: Bool = true) throws -> Timing {
        try submit { e in
            gemm(e, swizzle, x: x, w: weights.gate, y: state.gate, n: n, m: 12288, k: 4096)
            gemm(e, swizzle, x: x, w: weights.up, y: state.up, n: n, m: 12288, k: 4096)
            e.memoryBarrier(scope: .buffers); e.setComputePipelineState(pipelines["silu"]!)
            for (i, b) in [state.gate, state.up, state.activation].enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
            var count = UInt64(n * 12288); e.setBytes(&count, length: 8, index: 3)
            e.dispatchThreadgroups(MTLSize(width: (Int(count) + 127) / 128, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1)); dispatches += 1
            gemm(e, swizzle, x: state.activation, w: weights.down, y: state.output, n: n, m: 4096, k: 12288, acc: acc)
        }
    }
}
final class Weights {
    let gate, up, down: Buffer
    let hashes: [String]
    var buffers: [Buffer] { [gate, up, down] }
    init(_ device: MTLDevice) throws {
        let manifest = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("weights.json"))) as! [String: Any]
        let entries = manifest["tensors"] as! [[String: Any]]
        func read(_ file: String) throws -> Buffer {
            try autoreleasepool {
                let entry = entries.first { $0["half_file"] as? String == file }!
                let data = try Data(contentsOf: assets.appendingPathComponent(file), options: .mappedIfSafe)
                try require(data.count == 4096 * 12288 * 2 && hash(data) == entry["half_sha256"] as! String, "weight hash/size differs")
                let buffer = try Buffer(device, data.count)
                data.withUnsafeBytes { buffer.pointer.copyMemory(from: $0.baseAddress!, byteCount: data.count) }
                return buffer
            }
        }
        gate = try read("gate.f16"); up = try read("up.f16"); down = try read("down.f16")
        hashes = [gate, up, down].map { $0.digest() }
        try emit(["kind": "weights", "manifest": manifest, "guarded_buffer_sha256": hashes, "full_model_loaded": false])
    }
    func unchanged() throws {
        for (i, b) in buffers.enumerated() { try b.guards(); try require(b.digest() == hashes[i], "immutable real weight changed") }
    }
}
final class State {
    let gate, up, activation, output: Buffer
    var buffers: [Buffer] { [gate, up, activation, output] }
    init(_ device: MTLDevice, _ n: Int) throws {
        gate = try Buffer(device, n * 12288 * 4); up = try Buffer(device, n * 12288 * 4)
        activation = try Buffer(device, n * 12288 * 2); output = try Buffer(device, n * 4096 * 4)
    }
    func reset(_ acc: Bool = true) {
        gate.poisonFloat(); up.poisonFloat(); activation.poisonHalf()
        if acc { residual(output) } else { output.poisonFloat() }
    }
    func guards() throws { for b in buffers { try b.guards() } }
}
func small(_ runner: Runner, n: Int, m: Int, k: Int, acc: Bool) throws {
    try autoreleasepool {
        let x = try Buffer(runner.device, n * k * 2), w = try Buffer(runner.device, m * k * 2)
        let a = try Buffer(runner.device, n * m * 4), b = try Buffer(runner.device, n * m * 4)
        fillHalf(x, width: k, seed: 19, scale: 1); fillHalf(w, width: k, seed: 41, scale: 1 / sqrt(Float(k)))
        let before = [x.digest(), w.digest()]
        if acc { residual(a); residual(b) } else { a.poisonFloat(); b.poisonFloat() }
        let xp = x.pointer.assumingMemoryBound(to: Float16.self), wp = w.pointer.assumingMemoryBound(to: Float16.self)
        var expected = [Double](repeating: 0, count: n * m)
        for row in 0..<n { for col in 0..<m {
            var sum = acc ? Double(a.pointer.assumingMemoryBound(to: Float.self)[row * m + col]) : 0
            for j in 0..<k { sum += Double(xp[row * k + j]) * Double(wp[col * k + j]) }
            expected[row * m + col] = sum
        }}
        _ = try runner.submit { runner.gemm($0, false, x: x, w: w, y: a, n: n, m: m, k: k, acc: acc) }
        _ = try runner.submit { runner.gemm($0, true, x: x, w: w, y: b, n: n, m: m, k: k, acc: acc) }
        let count = try exact(a, b)
        var maximum = 0.0
        let p = a.pointer.assumingMemoryBound(to: Float.self)
        for i in 0..<count {
            let error = abs(Double(p[i]) - expected[i]); maximum = max(maximum, error)
            try require(error <= 0.00003 * max(1, abs(expected[i])), "baseline f64 oracle exceeded existing 3e-5 scaled bound")
        }
        try require([x.digest(), w.digest()] == before, "small input changed")
        try emit(["kind": "small_correctness", "n": n, "m": m, "k": k, "accumulate": acc, "baseline_kernel": kernel(false, m, k, acc), "values": count, "bit_differences": 0, "f64_max_abs": maximum, "passed": true, "redzones_unchanged": true, "inputs_unchanged": true])
    }
}
func weighted(_ runner: Runner, _ weights: Weights, n: Int, acc: Bool) throws {
    try autoreleasepool {
        let x = try Buffer(runner.device, n * 4096 * 2)
        fillHalf(x, width: 4096, seed: 19, scale: 1)
        let initial = x.digest(), a = try State(runner.device, n), b = try State(runner.device, n)
        a.reset(acc); b.reset(acc)
        _ = try runner.ffn(false, weights, x, a, n: n, acc: acc)
        _ = try runner.ffn(true, weights, x, b, n: n, acc: acc)
        let gate = try exact(a.gate, b.gate), up = try exact(a.up, b.up)
        let activation = try exact(a.activation, b.activation, half: true), down = try exact(a.output, b.output)
        try require(x.digest() == initial, "real fixture input changed")
        try weights.unchanged(); try runner.memory("weighted_\(n)_\(acc)")
        try emit(["kind": "weighted_correctness", "n": n, "accumulate": acc, "gate_values": gate, "up_values": up, "activation_values": activation, "down_values": down, "bit_differences": 0, "passed": true, "inputs_unchanged": true, "redzones_unchanged": true, "baseline_kernels": [kernel(false, 12288, 4096, false), kernel(false, 4096, 12288, acc)]])
    }
}
func timed(_ runner: Runner, _ weights: Weights, n: Int) throws {
    try autoreleasepool {
        let x = try Buffer(runner.device, n * 4096 * 2)
        fillHalf(x, width: 4096, seed: 19, scale: 1)
        let initial = x.digest(), baseline = try State(runner.device, n), state = try State(runner.device, n)
        baseline.reset(); _ = try runner.ffn(false, weights, x, baseline, n: n)
        let expectedHashes = baseline.buffers.map { $0.digest() }
        for round in 0..<5 { for (slot, swizzle) in [false, true, true, false].enumerated() {
            for scope in ["gate", "up", "down_accumulate", "full_ffn_accumulate"] {
                state.reset()
                let timing: Timing
                if scope == "full_ffn_accumulate" {
                    timing = try runner.ffn(swizzle, weights, x, state, n: n)
                } else {
                    timing = try runner.submit { e in
                        if scope == "gate" { runner.gemm(e, swizzle, x: x, w: weights.gate, y: state.gate, n: n, m: 12288, k: 4096) }
                        else if scope == "up" { runner.gemm(e, swizzle, x: x, w: weights.up, y: state.up, n: n, m: 12288, k: 4096) }
                        else { runner.gemm(e, swizzle, x: baseline.activation, w: weights.down, y: state.output, n: n, m: 4096, k: 12288, acc: true) }
                    }
                }
                try state.guards()
                if scope == "gate" { _ = try exact(baseline.gate, state.gate) }
                else if scope == "up" { _ = try exact(baseline.up, state.up) }
                else if scope == "down_accumulate" { _ = try exact(baseline.output, state.output) }
                else {
                    _ = try exact(baseline.gate, state.gate); _ = try exact(baseline.up, state.up)
                    _ = try exact(baseline.activation, state.activation, half: true)
                    _ = try exact(baseline.output, state.output)
                }
                try emit(["kind": "timing", "output_bitwise_passed": true, "variant": swizzle ? "swizzle" : "baseline", "n": n, "round": round, "slot": slot, "scope": scope, "gpu_ms": timing.gpu * 1000, "wall_ms": timing.wall * 1000, "load_average": loadAverage(), "current_allocated_bytes": runner.device.currentAllocatedSize])
            }
        }}
        try require(x.digest() == initial && baseline.buffers.map { $0.digest() } == expectedHashes, "timed input or immutable expected output changed")
        try weights.unchanged(); try runner.memory("timed_\(n)")
    }
}
do {
    let runner = try Runner()
    for (n, m, k) in [(1, 1, 256), (3, 7, 4096), (7, 32, 4096), (65, 129, 64), (257, 65, 256)] {
        for acc in [false, true] { try small(runner, n: n, m: m, k: k, acc: acc) }
    }
    let weights = try Weights(runner.device)
    try runner.memory("weights_loaded")
    for n in [65, 512, 1024, 2048] { try weighted(runner, weights, n: n, acc: true) }
    try weighted(runner, weights, n: 65, acc: false)
    try emit(["kind": "correctness_complete", "small_checks": 10, "weighted_checks": 5, "full_output_bitwise": true])
    if benchmark { for n in [512, 1024, 2048] { try timed(runner, weights, n: n) } }
    try require(commands == (benchmark ? 273 : 30) && dispatches == (benchmark ? 492 : 60), "dispatch or command count differs from frozen plan")
    try emit(["kind": "complete", "passed": true, "timing_included": benchmark, "commands": commands, "dispatches": dispatches, "explicit_peak_bytes": peakBytes, "current_allocated_bytes": runner.device.currentAllocatedSize, "seconds": ProcessInfo.processInfo.systemUptime - started, "full_model_loaded": false])
} catch {
    try? emit(["kind": "failure", "error": String(describing: error), "commands": commands, "dispatches": dispatches, "seconds": ProcessInfo.processInfo.systemUptime - started])
    exit(1)
}
