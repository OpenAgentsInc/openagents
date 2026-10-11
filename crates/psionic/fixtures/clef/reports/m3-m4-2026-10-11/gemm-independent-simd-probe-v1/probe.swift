// Four independent 32 by 32 SIMD tiles, with one fixed 128-wide K step.
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
var liveBytes = 0, peakBytes = 0, expectedBytes = 0, peakExpectedBytes = 0, peakCombinedBytes = 0, transientAssetBytes = 0, commands = 0, dispatches = 0
func memoryCheck(_ device: MTLDevice) throws {
    let combined = device.currentAllocatedSize + expectedBytes + transientAssetBytes
    peakCombinedBytes = max(peakCombinedBytes, combined)
    try require(combined <= driverLimit, "Metal plus owned immutable expectations exceed 1.5 GiB")
}
func timeCheck() throws {
    try require(ProcessInfo.processInfo.systemUptime - started < 50, "internal 50-second deadline reached")
}
final class Buffer {
    let metal: MTLBuffer
    let bytes: Int
    var pointer: UnsafeMutableRawPointer { metal.contents().advanced(by: pad) }
    init(_ device: MTLDevice, _ bytes: Int) throws {
        try require(bytes > 0 && bytes <= dataLimit - 2 * pad, "invalid buffer size")
        let total = bytes + 2 * pad
        try require(liveBytes <= dataLimit - total, "explicit live buffers exceed 1 GiB")
        try require(device.currentAllocatedSize + expectedBytes + transientAssetBytes + total <= driverLimit, "buffer allocation would exceed combined limit")
        guard let allocation = device.makeBuffer(length: total, options: .storageModeShared) else { throw Failure("buffer allocation failed") }
        self.metal = allocation; self.bytes = bytes
        liveBytes += total; peakBytes = max(peakBytes, liveBytes)
        memset(allocation.contents(), 0xa5, total)
        try memoryCheck(device)
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
// This exact shape table mirrors the retained ClefGemmPlan::new WeightShapes match.
func tile(_ m: Int, _ k: Int) -> Int {
    if m == 32 && k == 4096 { return 32 }
    if (k == 4096 && [4096, 8192, 12288].contains(m)) || (m == 4096 && k == 12288) { return 128 }
    if (k == 5120 && [6144, 10240, 12288, 17408].contains(m)) || (m == 5120 && [6144, 17408].contains(k)) { return 128 }
    return 64
}
func kernel(_ independent: Bool, _ m: Int, _ k: Int, _ accumulate: Bool, forceWide: Bool = false) -> String {
    let t = forceWide ? 128 : tile(m, k)
    let stem = independent ? "independent_simd" : (t == 64 ? "clef_gemm" : "clef_gemm_64x\(t)")
    return stem + (accumulate ? "_accumulate" : "")
}
func validateShape(n: Int, m: Int, k: Int) throws {
    try require(n > 0 && n <= 2048 && m > 0 && m <= 12288 && k > 0 && k <= 12288, "shape exceeds frozen probe bounds")
}
struct Timing { let gpu, wall: Double }
final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    var pipelines = [String: MTLComputePipelineState]()
    init() throws {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { throw Failure("Metal unavailable") }
        self.device = device; self.queue = queue
        let library = try device.makeLibrary(URL: root.appendingPathComponent("kernels.metallib"))
        for (name, threads) in [("clef_gemm",128),("clef_gemm_accumulate",128),
                                ("clef_gemm_64x32",128),("clef_gemm_64x32_accumulate",128),
                                ("clef_gemm_64x128",128),("clef_gemm_64x128_accumulate",128),
                                ("independent_simd",128),("independent_simd_accumulate",128)] {
            guard let function = library.makeFunction(name: name) else { throw Failure("kernel missing: \(name)") }
            let pipeline = try device.makeComputePipelineState(function: function)
            try require(pipeline.threadExecutionWidth == 32 && pipeline.maxTotalThreadsPerThreadgroup >= threads, "pipeline thread count unsupported")
            try require(pipeline.staticThreadgroupMemoryLength <= device.maxThreadgroupMemoryLength, "pipeline shared memory unsupported")
            pipelines[name] = pipeline
        }
        guard let silu = library.makeFunction(name: "clef_silu_mul_to_f16") else { throw Failure("SiLU kernel missing") }
        let pipeline = try device.makeComputePipelineState(function: silu)
        try require(pipeline.maxTotalThreadsPerThreadgroup >= 128, "SiLU threads unsupported")
        pipelines["silu"] = pipeline
        try emit(["kind": "device", "name": device.name, "current_allocated_bytes": device.currentAllocatedSize, "recommended_working_set_bytes": device.recommendedMaxWorkingSetSize, "explicit_buffer_limit_bytes": dataLimit, "driver_allocation_limit_bytes": driverLimit, "load_average": loadAverage()])
    }
    func memory(_ stage: String) throws {
        try memoryCheck(device)
        try emit(["kind": "allocation", "stage": stage, "current_allocated_bytes": device.currentAllocatedSize, "explicit_live_bytes": liveBytes, "explicit_peak_bytes": peakBytes, "owned_expected_bytes": expectedBytes, "combined_driver_and_expected_bytes": device.currentAllocatedSize + expectedBytes + transientAssetBytes])
    }
    func gemm(_ e: MTLComputeCommandEncoder, _ independent: Bool, x: Buffer, w: Buffer, y: Buffer, n: Int, m: Int, k: Int, acc: Bool = false, forceWide: Bool = false) throws {
        try validateShape(n: n, m: m, k: k)
        let columnsPerTile = independent ? 64 : (forceWide ? 128 : tile(m, k))
        try require(x.bytes == n * k * 2 && w.bytes == m * k * 2 && y.bytes == n * m * 4, "binding lengths do not match the shape")
        e.memoryBarrier(scope: .buffers)
        e.setComputePipelineState(pipelines[kernel(independent, m, k, acc, forceWide: forceWide)]!)
        for (i, b) in [x, w, y].enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
        var args = SIMD4<UInt32>(UInt32(n), UInt32(m), UInt32(k), 0)
        e.setBytes(&args, length: 16, index: 3)
        let grid = MTLSize(width: (m + columnsPerTile - 1) / columnsPerTile, height: (n + 63) / 64, depth: 1)
        e.dispatchThreadgroups(grid, threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
        dispatches += 1
    }
    func submit(_ f: (MTLComputeCommandEncoder) throws -> Void) throws -> Timing {
        try timeCheck()
        return try autoreleasepool {
            let began = ProcessInfo.processInfo.systemUptime
            guard let cb = queue.makeCommandBuffer(), let e = cb.makeComputeCommandEncoder(dispatchType: .concurrent) else { throw Failure("command allocation failed") }
            try f(e); e.endEncoding(); cb.commit(); cb.waitUntilCompleted(); commands += 1
            try require(cb.status == .completed, "GPU command failed: \(String(describing: cb.error))")
            try memoryCheck(device)
            let gpu = cb.gpuEndTime - cb.gpuStartTime
            try require(gpu.isFinite && gpu > 0, "invalid GPU timestamp")
            return Timing(gpu: gpu, wall: ProcessInfo.processInfo.systemUptime - began)
        }
    }
    func ffn(_ independent: Bool, _ weights: Weights, _ x: Buffer, _ state: State, n: Int, acc: Bool = true) throws -> Timing {
        try submit { e in
            try gemm(e, independent, x: x, w: weights.gate, y: state.gate, n: n, m: 12288, k: 4096)
            try gemm(e, independent, x: x, w: weights.up, y: state.up, n: n, m: 12288, k: 4096)
            e.memoryBarrier(scope: .buffers); e.setComputePipelineState(pipelines["silu"]!)
            for (i, b) in [state.gate, state.up, state.activation].enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
            var count = UInt64(n * 12288); e.setBytes(&count, length: 8, index: 3)
            e.dispatchThreadgroups(MTLSize(width: (Int(count) + 127) / 128, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1)); dispatches += 1
            try gemm(e, independent, x: state.activation, w: weights.down, y: state.output, n: n, m: 4096, k: 12288, acc: acc)
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
                transientAssetBytes += 4096 * 12288 * 2
                defer { transientAssetBytes -= 4096 * 12288 * 2 }
                try memoryCheck(device)
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
// Collapse signed zero to the same ordered point; adjacent finite half values
// on either side then differ by one integer step.
func halfOrder(_ bits: UInt16) -> Int {
    let magnitude = Int(bits & 0x7fff)
    return (bits & 0x8000) == 0 ? 0x8000 + magnitude : 0x8000 - magnitude
}
func halfULP(_ a: UInt16, _ b: UInt16) -> Int { abs(halfOrder(a) - halfOrder(b)) }
func activationAllowed(_ a: Float16, _ b: Float16) -> Bool {
    a.isFinite && b.isFinite && (abs(Double(a) - Double(b)) <= 0.001 || halfULP(a.bitPattern, b.bitPattern) <= 1)
}
func validateULPHelper() throws {
    let fixture = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("half-ulp-fixtures.json"))) as! [String: Any]
    let cases = fixture["cases"] as! [[String: Any]]
    for row in cases {
        let a = Float16(bitPattern: UInt16(row["a_bits"] as! Int)), b = Float16(bitPattern: UInt16(row["b_bits"] as! Int))
        try require(halfULP(a.bitPattern, b.bitPattern) == row["ulp_distance"] as! Int, "half ULP fixture distance differs")
        try require(activationAllowed(a, b) == row["allowed"] as! Bool, "half ULP fixture acceptance differs")
    }
    try emit(["kind": "half_ulp_self_test", "cases": cases.count, "passed": true, "criterion_version": 2])
}
func compare(_ a: Buffer, _ b: Buffer, half: Bool = false) throws -> [String: Any] {
    try require(a.bytes == b.bytes, "comparison length differs")
    try a.guards(); try b.guards()
    let count = a.bytes / (half ? 2 : 4)
    var maximum = 0.0, differences = 0, aboveAbsolute = 0, aboveULP = 0, maximumULP = 0, violations = 0, signedZeros = 0
    var first = [[String: Any]]()
    for i in 0..<count {
        let av: Double, bv: Double, ab: UInt32, bb: UInt32
        if half {
            let p = a.pointer.assumingMemoryBound(to: Float16.self)[i], q = b.pointer.assumingMemoryBound(to: Float16.self)[i]
            av = Double(p); bv = Double(q); ab = UInt32(p.bitPattern); bb = UInt32(q.bitPattern)
        } else {
            let p = a.pointer.assumingMemoryBound(to: Float.self)[i], q = b.pointer.assumingMemoryBound(to: Float.self)[i]
            av = Double(p); bv = Double(q); ab = p.bitPattern; bb = q.bitPattern
        }
        if !(av.isFinite && bv.isFinite) {
            try emit(["kind": "nonfinite_failure", "index": i, "half": half, "baseline_bits": ab, "candidate_bits": bb])
            throw Failure("nonfinite or unwritten output")
        }
        let error = abs(av - bv); maximum = max(maximum, error)
        let ulp = half ? halfULP(UInt16(ab), UInt16(bb)) : 0
        if half { maximumULP = max(maximumULP, ulp); if ulp > 1 { aboveULP += 1 } }
        if half && av == 0 && bv == 0 && ab != bb { signedZeros += 1 }
        if ab != bb { differences += 1 }
        if error > 0.001 { aboveAbsolute += 1 }
        if error > 0.001 && (!half || ulp > 1) {
            violations += 1
            if first.count < 8 { first.append(["index": i, "baseline": av, "candidate": bv, "error": error, "half_ulp_distance": ulp]) }
        }
    }
    let result: [String: Any] = ["values": count, "half": half, "max_abs": maximum, "bit_differences": differences, "above_0_001": aboveAbsolute, "half_max_ulp": maximumULP, "half_above_one_ulp": aboveULP, "signed_zero_pairs": signedZeros, "criterion_violations": violations, "criterion_version": 2, "first_failures": first, "passed": violations == 0]
    if violations > 0 {
        try emit(["kind": "numerical_failure", "comparison": result])
        throw Failure("full output exceeded its frozen pre-execution numerical bound")
    }
    return result
}
func residualValue(_ i: Int) -> Float { Float((i * 13 + 17) % 101 - 50) / 37 }
func oracle(_ xa: Buffer, _ xb: Buffer, _ w: Buffer, _ a: Buffer, _ b: Buffer, n: Int, m: Int, k: Int, acc: Bool, all: Bool) throws -> [String: Any] {
    let xp = xa.pointer.assumingMemoryBound(to: Float16.self), xq = xb.pointer.assumingMemoryBound(to: Float16.self)
    let wp = w.pointer.assumingMemoryBound(to: Float16.self)
    let ap = a.pointer.assumingMemoryBound(to: Float.self), bp = b.pointer.assumingMemoryBound(to: Float.self)
    let indices = all ? Array(0..<(n * m)) : (0..<96).map { s in s < 4 ? ((s % 2 == 0 ? 0 : n - 1) * m + (s < 2 ? 0 : m - 1)) : ((s * 97 + 5) % n) * m + (s * 997 + 11) % m }
    var maximumA = 0.0, maximumB = 0.0
    for index in indices {
        let row = index / m, col = index % m
        var expectedA = 0.0, expectedB = 0.0
        for j in 0..<k {
            let weight = Double(wp[col * k + j])
            expectedA += Double(xp[row * k + j]) * weight
            expectedB += Double(xq[row * k + j]) * weight
        }
        if acc { expectedA += Double(residualValue(index)); expectedB += Double(residualValue(index)) }
        let ea = abs(Double(ap[index]) - expectedA), eb = abs(Double(bp[index]) - expectedB)
        maximumA = max(maximumA, ea); maximumB = max(maximumB, eb)
        if !(ap[index].isFinite && bp[index].isFinite && ea <= 0.001 && eb <= 0.001) {
            try emit(["kind": "oracle_failure", "row": row, "column": col, "k": k, "expected_baseline": expectedA, "expected_candidate": expectedB, "baseline_bits": ap[index].bitPattern, "candidate_bits": bp[index].bitPattern, "baseline_error_text": String(ea), "candidate_error_text": String(eb)])
            throw Failure("f64 oracle exceeded the fixed absolute 0.001 bound")
        }
    }
    return ["sample_count": indices.count, "all_outputs": all, "baseline_max_abs": maximumA, "candidate_max_abs": maximumB, "passed": true]
}
func fixture(_ x: Buffer, _ w: Buffer, n: Int, m: Int, k: Int, kind: String) {
    fillHalf(x, width: k, seed: 19, scale: 0.125); fillHalf(w, width: k, seed: 41, scale: 1 / sqrt(Float(k)))
    let xp = x.pointer.assumingMemoryBound(to: Float16.self), wp = w.pointer.assumingMemoryBound(to: Float16.self)
    if kind == "zero" { xp.initialize(repeating: 0, count: n * k); wp.initialize(repeating: 0, count: m * k) }
    if kind == "correlated" {
        for i in 0..<(n * k) { xp[i] = Float16(Float(i % 17 - 8) / 64) }
        for i in 0..<(m * k) { wp[i] = Float16(Float(i % 17 - 8) / 128) }
    }
    if kind == "cancellation" {
        for i in 0..<(n * k) { xp[i] = Float16(0.25) }
        for i in 0..<(m * k) { wp[i] = Float16((i % 2 == 0 ? 1.0 : -1.0) / 64) }
    }
    if kind == "mixedrange" {
        for i in 0..<(n * k) { xp[i] = Float16(i % 29 == 0 ? 4 : Float(i % 7 - 3) / 1024) }
        for i in 0..<(m * k) { wp[i] = Float16(i % 31 == 0 ? 0.03125 : Float(i % 11 - 5) / 16384) }
    }
}
let smallCases: [(Int, Int, Int, String)] = [(1, 1, 1, "zero"), (3, 7, 127, "nonsymmetric"), (7, 129, 257, "correlated"), (63, 1, 4096, "cancellation"), (64, 128, 65, "mixedrange"), (65, 129, 257, "nonsymmetric"), (257, 1, 12288, "nonsymmetric"), (1, 33, 4096, "mixedrange"), (33, 1, 12288, "cancellation")]
func small(_ runner: Runner, n: Int, m: Int, k: Int, kind: String, acc: Bool) throws {
    try autoreleasepool {
        try validateShape(n: n, m: m, k: k)
        let x = try Buffer(runner.device, n * k * 2), w = try Buffer(runner.device, m * k * 2)
        let a = try Buffer(runner.device, n * m * 4), b = try Buffer(runner.device, n * m * 4)
        fixture(x, w, n: n, m: m, k: k, kind: kind)
        let initial = [x.digest(), w.digest()]
        if acc { residual(a); residual(b) } else { a.poisonFloat(); b.poisonFloat() }
        _ = try runner.submit { try runner.gemm($0, false, x: x, w: w, y: a, n: n, m: m, k: k, acc: acc, forceWide: false) }
        _ = try runner.submit { try runner.gemm($0, true, x: x, w: w, y: b, n: n, m: m, k: k, acc: acc, forceWide: false) }
        let comparison = try compare(a, b)
        let reference = try oracle(x, x, w, a, b, n: n, m: m, k: k, acc: acc, all: true)
        try require([x.digest(), w.digest()] == initial, "small immutable input changed")
        try emit(["kind": "small_correctness", "n": n, "m": m, "k": k, "fixture": kind, "accumulate": acc, "baseline_kernel": kernel(false, m, k, acc, forceWide: false), "candidate_kernel": kernel(true, m, k, acc, forceWide: false), "comparison": comparison, "f64_oracle": reference, "passed": true, "guards_and_inputs_unchanged": true])
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
        let comparisons = ["gate": try compare(a.gate, b.gate), "up": try compare(a.up, b.up), "activation": try compare(a.activation, b.activation, half: true), "down": try compare(a.output, b.output)]
        // Down's independent oracle uses each variant's own half activation.
        let references = ["gate": try oracle(x, x, weights.gate, a.gate, b.gate, n: n, m: 12288, k: 4096, acc: false, all: false),
                          "up": try oracle(x, x, weights.up, a.up, b.up, n: n, m: 12288, k: 4096, acc: false, all: false),
                          "down": try oracle(a.activation, b.activation, weights.down, a.output, b.output, n: n, m: 4096, k: 12288, acc: acc, all: false)]
        try require(x.digest() == initial, "weighted immutable input changed")
        try weights.unchanged(); try a.guards(); try b.guards(); try runner.memory("weighted_\(n)_\(acc)")
        try emit(["kind": "weighted_correctness", "n": n, "accumulate": acc, "comparisons": comparisons, "f64_oracles": references, "passed": true, "guards_and_inputs_unchanged": true, "baseline_kernels": [kernel(false, 12288, 4096, false), kernel(false, 4096, 12288, acc)]])
    }
}
// Immutable owned CPU bytes keep Metal buffers below 1 GiB. Count these bytes
// together with currentAllocatedSize before any allocation or timed submission.
final class Expected {
    let values: [Data]
    let hashes: [String]
    let bytes: Int
    init(_ device: MTLDevice, _ state: State, only: Int? = nil) throws {
        let buffers = only.map { [state.buffers[$0]] } ?? state.buffers
        bytes = buffers.reduce(0) { $0 + $1.bytes }
        try require(device.currentAllocatedSize + expectedBytes + bytes <= driverLimit, "immutable expectations would exceed combined limit")
        values = buffers.map { Data(bytes: $0.pointer, count: $0.bytes) }
        hashes = values.map { hash($0) }
        expectedBytes += bytes; peakExpectedBytes = max(peakExpectedBytes, expectedBytes)
        try memoryCheck(device)
    }
    deinit { expectedBytes -= bytes }
    func unchanged() throws {
        try require(values.map { hash($0) } == hashes, "immutable expected bytes changed")
    }
    func check(_ index: Int, _ output: Buffer) throws -> [String: Any] {
        try output.guards()
        let expected = values[index]
        try require(expected.count == output.bytes, "expected output length differs")
        let identical = expected.withUnsafeBytes { memcmp($0.baseAddress!, output.pointer, output.bytes) == 0 }
        if !identical {
            try emit(["kind": "repeat_failure", "buffer_index": index, "expected_sha256": hashes[index], "actual_sha256": hash(Data(bytesNoCopy: output.pointer, count: output.bytes, deallocator: .none))])
            throw Failure("timed output differs from its fully checked immutable expectation")
        }
        return ["bytes": output.bytes, "bitwise_equal": true, "passed": true]
    }
}
func expectations(_ runner: Runner, _ weights: Weights, _ x: Buffer, n: Int) throws -> (Expected, Expected, Expected) {
    try autoreleasepool {
        let a = try State(runner.device, n), b = try State(runner.device, n)
        a.reset(); b.reset()
        _ = try runner.ffn(false, weights, x, a, n: n)
        _ = try runner.ffn(true, weights, x, b, n: n)
        let comparisons = ["gate": try compare(a.gate, b.gate), "up": try compare(a.up, b.up), "activation": try compare(a.activation, b.activation, half: true), "down": try compare(a.output, b.output)]
        let baseline = try Expected(runner.device, a), candidate = try Expected(runner.device, b)
        // Standalone down uses the common baseline half activation. Its candidate
        // expectation must not silently use the candidate full-FFN activation.
        residual(b.output)
        _ = try runner.submit { try runner.gemm($0, true, x: a.activation, w: weights.down, y: b.output, n: n, m: 4096, k: 12288, acc: true) }
        let downComparison = try compare(a.output, b.output)
        let downOracle = try oracle(a.activation, a.activation, weights.down, a.output, b.output, n: n, m: 4096, k: 12288, acc: true, all: false)
        let standalone = try Expected(runner.device, b, only: 3)
        try emit(["kind": "expectations_checked", "n": n, "comparisons": comparisons, "standalone_down": downComparison, "standalone_down_f64": downOracle, "baseline_sha256": baseline.hashes, "candidate_sha256": candidate.hashes, "standalone_down_sha256": standalone.hashes])
        return (baseline, candidate, standalone)
    }
}
func timed(_ runner: Runner, _ weights: Weights, n: Int) throws {
    try autoreleasepool {
        let x = try Buffer(runner.device, n * 4096 * 2)
        fillHalf(x, width: 4096, seed: 19, scale: 1)
        let initial = x.digest()
        let (baseline, candidate, standalone) = try expectations(runner, weights, x, n: n)
        let retainedBuffers = weights.buffers + [x]
        try require(liveBytes == retainedBuffers.reduce(0) { $0 + $1.bytes + 2 * pad }, "initial workspaces remain live before timing")
        try runner.memory("expectations_frozen_\(n)")
        let state = try State(runner.device, n)
        let commonActivation = try Buffer(runner.device, n * 12288 * 2)
        baseline.values[2].withUnsafeBytes { commonActivation.pointer.copyMemory(from: $0.baseAddress!, byteCount: commonActivation.bytes) }
        let activationHash = commonActivation.digest()
        for round in 0..<5 { for (slot, independent) in [false, true, true, false].enumerated() {
            for scope in ["gate", "up", "down_accumulate", "full_ffn_accumulate"] {
                state.reset()
                let timing: Timing
                if scope == "full_ffn_accumulate" {
                    timing = try runner.ffn(independent, weights, x, state, n: n)
                } else {
                    timing = try runner.submit { e in
                        if scope == "gate" { try runner.gemm(e, independent, x: x, w: weights.gate, y: state.gate, n: n, m: 12288, k: 4096) }
                        else if scope == "up" { try runner.gemm(e, independent, x: x, w: weights.up, y: state.up, n: n, m: 12288, k: 4096) }
                        else { try runner.gemm(e, independent, x: commonActivation, w: weights.down, y: state.output, n: n, m: 4096, k: 12288, acc: true) }
                    }
                }
                try state.guards(); try commonActivation.guards()
                let expected = independent ? candidate : baseline
                var repeats = [String: [String: Any]]()
                if scope == "gate" { repeats["gate"] = try expected.check(0, state.gate) }
                else if scope == "up" { repeats["up"] = try expected.check(1, state.up) }
                else if scope == "down_accumulate" { repeats["down"] = try (independent ? standalone : baseline).check(independent ? 0 : 3, state.output) }
                else { for (i, name) in ["gate", "up", "activation", "down"].enumerated() { repeats[name] = try expected.check(i, state.buffers[i]) } }
                try emit(["kind": "timing", "output_gate_passed": true, "repeats": repeats, "variant": independent ? "independent_simd" : "baseline", "n": n, "round": round, "slot": slot, "scope": scope, "gpu_ms": timing.gpu * 1000, "wall_ms": timing.wall * 1000, "load_average": loadAverage(), "current_allocated_bytes": runner.device.currentAllocatedSize, "owned_expected_bytes": expectedBytes])
            }
        }}
        try require(x.digest() == initial && commonActivation.digest() == activationHash, "timed immutable input changed")
        try baseline.unchanged(); try candidate.unchanged(); try standalone.unchanged()
        try weights.unchanged(); try runner.memory("timed_\(n)")
    }
}

do {
    for (n, m, k) in [(0,1,1),(2049,1,1),(1,0,1),(1,12289,1),(1,1,0),(1,1,12289)] {
        var refused = false
        do { try validateShape(n: n, m: m, k: k) } catch { refused = true }
        try require(refused, "invalid shape was accepted")
    }
    try validateULPHelper()
    let runner = try Runner()
    for (n, m, k, kind) in smallCases { for acc in [false, true] { try small(runner, n: n, m: m, k: k, kind: kind, acc: acc) } }
    let weights = try Weights(runner.device); try runner.memory("weights_loaded")
    for n in [65, 512, 1024, 2048] { try weighted(runner, weights, n: n, acc: true) }
    try weighted(runner, weights, n: 65, acc: false)
    try emit(["kind": "correctness_complete", "small_checks": 18, "weighted_checks": 5, "fixed_f32_absolute_error_bound": 0.001, "half_activation_absolute_or_ulp_bound": [0.001, 1], "criterion_version": 2])
    if benchmark { for n in [512, 1024, 2048] { try timed(runner, weights, n: n) } }
    try require(commands == (benchmark ? 295 : 46) && dispatches == (benchmark ? 523 : 76), "dispatch or command count differs from the frozen plan")
    try emit(["kind": "complete", "passed": true, "timing_included": benchmark, "commands": commands, "dispatches": dispatches, "explicit_peak_bytes": peakBytes, "expected_peak_bytes": peakExpectedBytes, "combined_driver_and_expected_peak_bytes": peakCombinedBytes, "current_allocated_bytes": runner.device.currentAllocatedSize, "seconds": ProcessInfo.processInfo.systemUptime - started, "full_model_loaded": false])
} catch {
    try? emit(["kind": "failure", "error": String(describing: error), "commands": commands, "dispatches": dispatches, "seconds": ProcessInfo.processInfo.systemUptime - started])
    exit(1)
}
