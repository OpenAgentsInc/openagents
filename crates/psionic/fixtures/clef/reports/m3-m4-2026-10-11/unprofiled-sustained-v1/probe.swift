// Owned command timestamps only; no profiler, model, or system changes.
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
try require(arguments.count == 4 && arguments[2] == "check", "Usage: probe DIRECTORY check ASSETS")
let root = URL(fileURLWithPath: arguments[1]), assets = URL(fileURLWithPath: arguments[3])
let started = ProcessInfo.processInfo.systemUptime
let dataLimit = 768 * 1024 * 1024
let driverLimit = 1024 * 1024 * 1024
let pad = 256
let poison = Float(bitPattern: 0x7fc00555)
var liveBytes = 0, peakBytes = 0, commands = 0, dispatches = 0
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
        try require(liveBytes <= dataLimit - total, "explicit live buffers exceed 768 MiB")
        guard let allocation = device.makeBuffer(length: total, options: [.storageModeShared, .hazardTrackingModeTracked]) else { throw Failure("buffer allocation failed") }
        try require(allocation.hazardTrackingMode == .tracked, "buffer hazard tracking required")
        self.metal = allocation; self.bytes = bytes
        liveBytes += total; peakBytes = max(peakBytes, liveBytes)
        memset(allocation.contents(), 0xa5, total)
        try require(device.currentAllocatedSize <= driverLimit, "Metal allocations exceed 1 GiB including private expansion")
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
let n = 2048, hidden = 4096, wide = 12288
let plannedBytes = 3 * hidden * wide * 2 + n * hidden * 2 + n * wide * 10 + 5 * n * hidden * 4 + 12 * 2 * pad
try require(plannedBytes < dataLimit, "fixed plan exceeds 768 MiB")
struct Pending {
    let phase, ordinal, slot, depth: Int
    let encodeStart, commitBefore, commitAfter: Double
    let command: MTLCommandBuffer
}
struct Row {
    let phase, ordinal, slot, depth: Int
    let encodeStart, commitBefore, commitAfter, retired, kernelStart, kernelEnd, gpuStart, gpuEnd: Double
    func json(_ origin: Double) -> [String: Any] {
        ["kind": "timing", "phase": phase, "variant": depth == 1 ? "wait_each" : "queued_four",
         "ordinal": ordinal, "slot": slot, "maximum_inflight": depth,
         "encode_start_seconds": encodeStart - origin, "commit_before_seconds": commitBefore - origin,
         "commit_after_seconds": commitAfter - origin, "retired_seconds": retired - origin,
         "driver_start_seconds": kernelStart - origin, "driver_end_seconds": kernelEnd - origin,
         "gpu_start_seconds": gpuStart - origin, "gpu_end_seconds": gpuEnd - origin,
         "gpu_ms": (gpuEnd - gpuStart) * 1000, "dispatches": 4]
    }
}
var traceRows = [Row]()
traceRows.reserveCapacity(512)
var traceOrigin = 0.0, emittedRows = 0
final class Scratch {
    let gate, up, activation: Buffer
    var buffers: [Buffer] { [gate, up, activation] }
    init(_ device: MTLDevice) throws {
        gate = try Buffer(device, n * wide * 4); up = try Buffer(device, n * wide * 4)
        activation = try Buffer(device, n * wide * 2)
    }
}
final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    let gemm, silu: MTLComputePipelineState
    init() throws {
        guard let d = MTLCreateSystemDefaultDevice(), let q = d.makeCommandQueue(maxCommandBufferCount: 4) else { throw Failure("Metal unavailable") }
        device = d; queue = q
        let library = try d.makeLibrary(URL: root.appendingPathComponent("kernels.metallib"))
        guard let g = library.makeFunction(name: "clef_gemm_64x128"), let s = library.makeFunction(name: "clef_silu_mul_to_f16") else { throw Failure("production kernels missing") }
        gemm = try d.makeComputePipelineState(function: g); silu = try d.makeComputePipelineState(function: s)
        for p in [gemm, silu] {
            try require(p.threadExecutionWidth == 32 && p.maxTotalThreadsPerThreadgroup >= 128, "unsupported pipeline threads")
            try require(p.staticThreadgroupMemoryLength <= d.maxThreadgroupMemoryLength, "unsupported pipeline threadgroup memory")
        }
        try emit(["kind": "device", "name": d.name, "current_allocated_bytes": d.currentAllocatedSize,
                  "explicit_limit_bytes": dataLimit, "planned_explicit_bytes": plannedBytes,
                  "driver_point_limit_bytes": driverLimit, "load_average": loadAverage(),
                  "profiled": false, "counter_sampling": false, "hazard_tracking": "explicit tracked; one queue",
                  "thermal_state_observed": ProcessInfo.processInfo.thermalState.rawValue,
                  "low_power_mode_observed": ProcessInfo.processInfo.isLowPowerModeEnabled])
    }
    func project(_ e: MTLComputeCommandEncoder, _ x: Buffer, _ w: Buffer, _ y: Buffer, n: Int, m: Int, k: Int) {
        e.memoryBarrier(scope: .buffers); e.setComputePipelineState(gemm)
        for (i, b) in [x, w, y].enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
        var dims = SIMD4<UInt32>(UInt32(n), UInt32(m), UInt32(k), 0)
        e.setBytes(&dims, length: 16, index: 3)
        e.dispatchThreadgroups(MTLSize(width: (m + 127) / 128, height: (n + 63) / 64, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
        dispatches += 1
    }
    func encode(_ phase: Int, _ ordinal: Int, _ slot: Int, _ depth: Int, weights: Weights, x: Buffer, scratch: Scratch, output: Buffer) throws -> Pending {
        try timeCheck()
        let began = ProcessInfo.processInfo.systemUptime
        guard let cb = queue.makeCommandBuffer(), let e = cb.makeComputeCommandEncoder(dispatchType: .concurrent) else { throw Failure("command allocation failed") }
        cb.label = "owned-ffn-\(phase)-\(ordinal)"
        project(e, x, weights.gate, scratch.gate, n: n, m: wide, k: hidden)
        project(e, x, weights.up, scratch.up, n: n, m: wide, k: hidden)
        e.memoryBarrier(scope: .buffers); e.setComputePipelineState(silu)
        for (i, b) in [scratch.gate, scratch.up, scratch.activation].enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
        var count = UInt64(n * wide); e.setBytes(&count, length: 8, index: 3)
        e.dispatchThreadgroups(MTLSize(width: (Int(count) + 127) / 128, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1)); dispatches += 1
        project(e, scratch.activation, weights.down, output, n: n, m: hidden, k: wide)
        e.endEncoding()
        let before = ProcessInfo.processInfo.systemUptime
        cb.commit(); commands += 1
        return Pending(phase: phase, ordinal: ordinal, slot: slot, depth: depth, encodeStart: began, commitBefore: before, commitAfter: ProcessInfo.processInfo.systemUptime, command: cb)
    }
    func retire(_ t: Pending) throws -> Row {
        t.command.waitUntilCompleted()
        let retired = ProcessInfo.processInfo.systemUptime
        try require(t.command.status == .completed, "GPU command failed: \(String(describing: t.command.error))")
        let gs = t.command.gpuStartTime, ge = t.command.gpuEndTime
        try require(gs.isFinite && ge.isFinite && ge > gs && gs > 0, "invalid GPU timestamps")
        let row = Row(phase: t.phase, ordinal: t.ordinal, slot: t.slot, depth: t.depth, encodeStart: t.encodeStart,
                   commitBefore: t.commitBefore, commitAfter: t.commitAfter, retired: retired,
                   kernelStart: t.command.kernelStartTime, kernelEnd: t.command.kernelEndTime, gpuStart: gs, gpuEnd: ge)
        if t.phase >= 0 { traceRows.append(row) }
        return row
    }
}
func finite(_ buffer: Buffer, half: Bool = false) throws {
    try buffer.guards()
    if half {
        let p = buffer.pointer.assumingMemoryBound(to: Float16.self)
        for i in 0..<(buffer.bytes / 2) { try require(p[i].isFinite, "nonfinite half value") }
    } else {
        let p = buffer.pointer.assumingMemoryBound(to: Float.self)
        for i in 0..<(buffer.bytes / 4) { try require(p[i].isFinite, "nonfinite float value") }
    }
}
func oracle(_ runner: Runner) throws {
    try autoreleasepool {
        let rows = 3, cols = 7, k = hidden
        let x = try Buffer(runner.device, rows * k * 2), w = try Buffer(runner.device, cols * k * 2), y = try Buffer(runner.device, rows * cols * 4)
        fillHalf(x, width: k, seed: 19, scale: 1); fillHalf(w, width: k, seed: 41, scale: 1 / sqrt(Float(k))); y.poisonFloat()
        let initial = [x.digest(), w.digest()]
        guard let cb = runner.queue.makeCommandBuffer(), let e = cb.makeComputeCommandEncoder(dispatchType: .concurrent) else { throw Failure("oracle command allocation failed") }
        runner.project(e, x, w, y, n: rows, m: cols, k: k); e.endEncoding(); cb.commit(); commands += 1; cb.waitUntilCompleted()
        try require(cb.status == .completed, "oracle GPU command failed")
        let xp = x.pointer.assumingMemoryBound(to: Float16.self), wp = w.pointer.assumingMemoryBound(to: Float16.self), yp = y.pointer.assumingMemoryBound(to: Float.self)
        var maxError = 0.0
        for row in 0..<rows { for col in 0..<cols {
            var value = 0.0
            for i in 0..<k { value += Double(xp[row * k + i]) * Double(wp[col * k + i]) }
            let error = abs(Double(yp[row * cols + col]) - value)
            try require(yp[row * cols + col].isFinite && error <= 0.00003 * max(1, abs(value)), "f64 oracle exceeded 3e-5 scaled bound")
            maxError = max(maxError, error)
        }}
        try x.guards(); try w.guards(); try y.guards(); try require(initial == [x.digest(), w.digest()], "oracle inputs changed")
        try emit(["kind": "oracle", "passed": true, "values": rows * cols, "f64_max_abs": maxError])
    }
}
do {
    let runner = try Runner(); try oracle(runner)
    let weights = try Weights(runner.device), x = try Buffer(runner.device, n * hidden * 2), scratch = try Scratch(runner.device)
    let reference = try Buffer(runner.device, n * hidden * 4)
    let outputs = try (0..<4).map { _ in try Buffer(runner.device, n * hidden * 4) }
    try require(liveBytes == plannedBytes, "allocated buffers differ from fixed plan")
    fillHalf(x, width: hidden, seed: 19, scale: 1)
    scratch.gate.poisonFloat(); scratch.up.poisonFloat(); scratch.activation.poisonHalf(); reference.poisonFloat()
    _ = try autoreleasepool { try runner.retire(runner.encode(-1, 0, 0, 1, weights: weights, x: x, scratch: scratch, output: reference)) }
    try finite(scratch.gate); try finite(scratch.up); try finite(scratch.activation, half: true); try finite(reference)
    let expectedScratch = scratch.buffers.map { $0.digest() }, expectedReference = reference.digest(), initialX = x.digest()
    try weights.unchanged()
    var allRows = [Row](); allRows.reserveCapacity(512)
    let origin = ProcessInfo.processInfo.systemUptime
    traceOrigin = origin
    for (phase, depth) in [1, 4, 4, 1].enumerated() {
        try timeCheck()
        for output in outputs { output.poisonFloat() }
        var pending = [Pending](); pending.reserveCapacity(4)
        var phaseRows = [Row](); phaseRows.reserveCapacity(128)
        let phaseStarted = ProcessInfo.processInfo.systemUptime
        for ordinal in 0..<128 {
            // Retire before reusing the output slot. Tracked shared scratch
            // also preserves WAW and WAR dependencies between commands.
            if pending.count == depth { phaseRows.append(try runner.retire(pending.removeFirst())) }
            let command = try autoreleasepool { try runner.encode(phase, ordinal, ordinal % depth, depth, weights: weights, x: x, scratch: scratch, output: outputs[ordinal % depth]) }
            pending.append(command)
            try require(pending.count <= depth && pending.count <= 4, "inflight bound exceeded")
        }
        while !pending.isEmpty { phaseRows.append(try runner.retire(pending.removeFirst())) }
        let phaseEnded = ProcessInfo.processInfo.systemUptime
        try require(phaseRows.count == 128, "phase lost a command")
        for b in [scratch.gate, scratch.up] { try finite(b) }; try finite(scratch.activation, half: true)
        try require(scratch.buffers.map { $0.digest() } == expectedScratch, "complete intermediate output bits changed")
        var checked = 0
        for output in outputs.prefix(depth) { checked += try exact(reference, output) }
        for output in outputs.dropFirst(depth) { try output.guards() }
        try require(reference.digest() == expectedReference && x.digest() == initialX, "immutable reference or activation changed")
        try weights.unchanged()
        try require(runner.device.currentAllocatedSize <= driverLimit, "Metal allocation limit exceeded")
        allRows.append(contentsOf: phaseRows)
        try emit(["kind": "phase", "phase": phase, "variant": depth == 1 ? "wait_each" : "queued_four", "commands": phaseRows.count,
                  "stream_wall_ms": (phaseEnded - phaseStarted) * 1000, "full_final_values_checked": checked,
                  "intermediate_values_hashed": n * wide * 3, "bitwise_passed": true, "sources_unchanged": true,
                  "explicit_live_bytes": liveBytes, "driver_allocated_bytes": runner.device.currentAllocatedSize,
                  "load_average": loadAverage(), "thermal_state_observed": ProcessInfo.processInfo.thermalState.rawValue,
                  "low_power_mode_observed": ProcessInfo.processInfo.isLowPowerModeEnabled])
        for row in phaseRows { try emit(row.json(origin)); emittedRows += 1 }
    }
    try require(allRows.count == 512 && commands == 514 && dispatches == 2053, "frozen command count changed")
    try emit(["kind": "complete", "passed": true, "timed_commands": allRows.count, "total_commands": commands, "total_dispatches": dispatches,
              "explicit_peak_bytes": peakBytes, "seconds": ProcessInfo.processInfo.systemUptime - started,
              "profiled": false, "counter_sampling": false, "full_model_loaded": false,
              "scope": "same current FFN kernels, down beta0; one versus four queued commands; phase-boundary CPU checks are outside each stream"])
} catch {
    for row in traceRows.dropFirst(emittedRows) { try? emit(row.json(traceOrigin)) }
    try? emit(["kind": "failure", "error": String(describing: error), "commands": commands, "dispatches": dispatches,
               "seconds": ProcessInfo.processInfo.systemUptime - started])
    exit(1)
}
