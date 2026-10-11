// Diagnostic host only. The previously compiled Metal library stays unchanged.
import Foundation
import Metal
import CryptoKit
import Darwin

struct Failure: Error, CustomStringConvertible {
    let description: String
    init(_ message: String) { description = message }
}
func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw Failure(message) }
}
func sha(_ data: Data) -> String {
    SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
}
func emit(_ value: [String: Any]) throws {
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]))
    FileHandle.standardOutput.write(Data([10]))
}
func save(_ data: Data, to url: URL) throws {
    try require(!FileManager.default.fileExists(atPath: url.path), "diagnostic output already exists")
    try data.write(to: url, options: .withoutOverwriting)
    let file = try FileHandle(forWritingTo: url)
    try file.synchronize(); try file.close()
}
let pinnedLibraryHash = "eab78a332c74906ef892664845785dc2d555c8239c97d17a6633e5dacdfdb12a"
let pad = 256, explicitLimit = 1024 * 1024
let objects = 8, lanes = 32, slots = 64, stride = 257
let recordBytes = 8 * 32 * 257 * 4
var liveBytes = 0, peakBytes = 0, commands = 0, dispatches = 0
var activeStart: Double? = nil
func deadline() throws {
    if let start = activeStart {
        try require(ProcessInfo.processInfo.systemUptime - start < 10, "internal 10-second diagnostic deadline reached")
    }
}
final class Buffer {
    let metal: MTLBuffer
    let bytes: Int
    var pointer: UnsafeMutableRawPointer { metal.contents().advanced(by: pad) }
    init(_ device: MTLDevice, _ bytes: Int) throws {
        try require(bytes > 0 && bytes + 2 * pad < explicitLimit, "buffer size exceeds diagnostic bound")
        try require(liveBytes + bytes + 2 * pad < explicitLimit, "explicit buffers exceed diagnostic bound")
        guard let buffer = device.makeBuffer(length: bytes + 2 * pad, options: .storageModeShared) else {
            throw Failure("buffer allocation failed")
        }
        self.metal = buffer; self.bytes = bytes
        liveBytes += bytes + 2 * pad; peakBytes = max(peakBytes, liveBytes)
        memset(buffer.contents(), 0xa5, bytes + 2 * pad)
    }
    deinit { liveBytes -= bytes + 2 * pad }
    func digest() -> String {
        sha(Data(bytes: metal.contents(), count: bytes + 2 * pad))
    }
    func guards() throws {
        let p = metal.contents().assumingMemoryBound(to: UInt8.self)
        for i in 0..<pad {
            try require(p[i] == 0xa5 && p[pad + bytes + i] == 0xa5, "buffer redzone changed")
        }
    }
}

func exportCase(_ device: MTLDevice, _ queue: MTLCommandQueue, _ pipeline: MTLComputePipelineState,
                output: URL, side: Int) throws {
    try autoreleasepool {
        try deadline()
        let name = side == 1 ? "tail-1" : "interior-32"
        let x = try Buffer(device, side * side * 2)
        let weight = try Buffer(device, side * side * 2)
        let seed = try Buffer(device, side * side * 4)
        let records = try Buffer(device, recordBytes)
        let xp = x.pointer.assumingMemoryBound(to: UInt16.self)
        let wp = weight.pointer.assumingMemoryBound(to: UInt16.self)
        let sp = seed.pointer.assumingMemoryBound(to: UInt32.self)
        for i in 0..<(side * side) {
            // Positive, normal finite halves; distinct ranges identify each source.
            xp[i] = UInt16(0x2000 + i)
            wp[i] = UInt16(0x3000 + i)
            sp[i] = UInt32(0x3f000000 + i)
        }
        memset(records.pointer, 0xff, records.bytes)
        let before = [x.digest(), weight.digest(), seed.digest()]
        try emit(["kind": "case_begin", "case": name, "dimensions": [side, side, side],
                  "base": [0, 0, 0], "input_guarded_sha256": before,
                  "explicit_live_bytes": liveBytes, "device_current_allocated_bytes": device.currentAllocatedSize])
        guard let cb = queue.makeCommandBuffer(), let encoder = cb.makeComputeCommandEncoder() else {
            throw Failure("command allocation failed")
        }
        encoder.setComputePipelineState(pipeline)
        for (index, buffer) in [x, weight, seed].enumerated() {
            encoder.setBuffer(buffer.metal, offset: pad, index: index)
        }
        var dims = SIMD4<UInt32>(UInt32(side), UInt32(side), UInt32(side), 0)
        var base = SIMD4<UInt32>(repeating: 0)
        encoder.setBytes(&dims, length: 16, index: 3)
        encoder.setBuffer(records.metal, offset: pad, index: 4)
        encoder.setBytes(&base, length: 16, index: 5)
        encoder.dispatchThreadgroups(MTLSize(width: 1, height: 1, depth: 1),
                                    threadsPerThreadgroup: MTLSize(width: 32, height: 1, depth: 1))
        dispatches += 1
        encoder.endEncoding(); cb.commit(); commands += 1; cb.waitUntilCompleted()
        try require(cb.status == .completed, "coordinate export command failed: \(String(describing: cb.error))")

        // Persist the complete raw buffer before validating or interpreting slots.
        let raw = Data(bytes: records.pointer, count: records.bytes)
        try save(raw, to: output.appendingPathComponent(name + ".u32"))
        let words = records.pointer.assumingMemoryBound(to: UInt32.self)
        var exported = [[String: Any]]()
        for object in 0..<objects {
            for lane in 0..<lanes {
                let offset = (object * lanes + lane) * stride
                var entries = [[String: Any]](), holes = [Int]()
                for slot in 0..<slots {
                    let start = offset + 1 + slot * 4
                    let values = (0..<4).map { words[start + $0] }
                    if values.allSatisfy({ $0 == UInt32.max }) { holes.append(slot) }
                    else {
                        entries.append(["slot": slot, "coordinate": [values[0], values[1]],
                                        "bits": values[2], "valid_flag": values[3]])
                    }
                }
                exported.append(["object": object, "lane": lane, "capacity": words[offset],
                                 "slots": entries, "sentinel_holes": holes])
            }
        }
        let document: [String: Any] = ["case": name, "dimensions": [side, side, side], "base": [0, 0, 0],
                                       "library_sha256": pinnedLibraryHash, "raw_sha256": sha(raw),
                                       "raw_byte_order": "little-endian UInt32", "objects": exported]
        let dump = try JSONSerialization.data(withJSONObject: document, options: [.sortedKeys])
        try save(dump, to: output.appendingPathComponent(name + ".json"))
        try emit(["kind": "slots_dumped", "case": name, "raw_bytes": raw.count,
                  "raw_sha256": sha(raw), "json_sha256": sha(dump), "interpretation_performed": false])

        // These are export-format and memory checks, not coordinate-shape assumptions.
        var counts = [Int](repeating: 0, count: objects), holeCount = 0
        for object in 0..<objects {
            for lane in 0..<lanes {
                let offset = (object * lanes + lane) * stride, capacity = Int(words[offset])
                try require(capacity > 0 && capacity <= slots, "cooperative capacity exceeds fixed export size")
                for slot in 0..<slots {
                    let start = offset + 1 + slot * 4
                    if words[start + 3] == UInt32.max {
                        try require((0..<4).allSatisfy { words[start + $0] == UInt32.max }, "slot hole was partially overwritten")
                        holeCount += 1
                    } else {
                        try require(slot < capacity && words[start + 3] == 1, "export slot validity is malformed")
                        let bits = words[start + 2]
                        if object < 6 {
                            try require(bits <= UInt32(UInt16.max) && Float16(bitPattern: UInt16(truncatingIfNeeded: bits)).isFinite,
                                        "exported half is nonfinite or has upper bits")
                        } else {
                            try require(Float(bitPattern: bits).isFinite, "exported destination seed is nonfinite")
                        }
                        counts[object] += 1
                    }
                }
            }
        }
        try require(counts.allSatisfy { $0 > 0 }, "an exported object has no valid slots")
        for buffer in [x, weight, seed, records] { try buffer.guards() }
        try require([x.digest(), weight.digest(), seed.digest()] == before, "immutable diagnostic input changed")
        try deadline()
        try emit(["kind": "case_complete", "case": name, "slots_by_object": counts,
                  "hole_slots": holeCount, "input_redzones_holes_passed": true,
                  "coordinate_mapping_accepted": false, "device_current_allocated_bytes": device.currentAllocatedSize])
    }
}

do {
    let args = CommandLine.arguments
    try require(args.count == 3, "Usage: exporter PINNED_METALLIB OUTPUT_DIRECTORY")
    let libraryURL = URL(fileURLWithPath: args[1]), output = URL(fileURLWithPath: args[2])
    try require(sha(Data(contentsOf: libraryURL)) == pinnedLibraryHash, "pinned library hash differs")
    guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else {
        throw Failure("Metal unavailable")
    }
    let library = try device.makeLibrary(URL: libraryURL)
    guard let function = library.makeFunction(name: "register_input_roundtrip") else { throw Failure("export kernel missing") }
    let pipeline = try device.makeComputePipelineState(function: function)
    try require(pipeline.threadExecutionWidth == 32 && pipeline.maxTotalThreadsPerThreadgroup >= 32,
                "export pipeline thread count is unsupported")
    try require(pipeline.staticThreadgroupMemoryLength <= device.maxThreadgroupMemoryLength,
                "export pipeline threadgroup storage is unsupported")
    activeStart = ProcessInfo.processInfo.systemUptime
    try emit(["kind": "diagnostic_start", "identity": "register-coordinate-diagnostic-v1", "device": device.name,
              "library_sha256": pinnedLibraryHash, "kernel": "register_input_roundtrip", "explicit_limit_bytes": explicitLimit,
              "device_current_allocated_bytes": device.currentAllocatedSize, "internal_deadline_seconds_after_startup": 10,
              "gemm_executed": false, "performance_timing_recorded": false])
    for side in [1, 32] { try exportCase(device, queue, pipeline, output: output, side: side) }
    try require(commands == 2 && dispatches == 2 && liveBytes == 0, "diagnostic counts or buffer lifetimes differ")
    try require(sha(Data(contentsOf: libraryURL)) == pinnedLibraryHash, "pinned library changed during export")
    try deadline()
    try emit(["kind": "complete", "cases": 2, "commands": commands, "dispatches": dispatches,
              "explicit_peak_bytes": peakBytes, "explicit_live_bytes": liveBytes,
              "gemm_executed": false, "performance_timing_recorded": false, "mapping_repaired": false])
} catch {
    try? emit(["kind": "failure", "message": String(describing: error), "commands": commands,
               "dispatches": dispatches, "explicit_peak_bytes": peakBytes])
    exit(1)
}
