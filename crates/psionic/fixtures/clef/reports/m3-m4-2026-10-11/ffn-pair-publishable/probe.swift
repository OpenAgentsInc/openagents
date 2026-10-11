// Synthetic FFN experiment. No model files, network, or application controls.
import Foundation
import Metal
import Darwin

struct ProbeError: Error, CustomStringConvertible {
    let description: String
    init(_ description: String) { self.description = description }
}

func emit(_ row: [String: Any]) throws {
    let data = try JSONSerialization.data(withJSONObject: row, options: [.sortedKeys])
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([10]))
}

func loadAverage() -> [Double] {
    var values = [Double](repeating: 0, count: 3)
    let count = values.withUnsafeMutableBufferPointer { getloadavg($0.baseAddress!, 3) }
    return count == 3 ? values : []
}

let arguments = CommandLine.arguments
guard arguments.count == 3 && ["check", "bench"].contains(arguments[2]) else {
    fputs("Usage: probe DIRECTORY check|bench\n", stderr)
    exit(2)
}
let directory = URL(fileURLWithPath: arguments[1])
let checkOnly = arguments[2] == "check"
let redzone = 64
let poison = Float16(bitPattern: 0x7e55)
let maximumDataBytes = 768 * 1024 * 1024

enum Mode: String, CaseIterable {
    case separate, fusedUp, fusedPair
}

struct Timing {
    let gpu: Double
    let wall: Double
}

final class Fixture {
    let n: Int, m: Int, k: Int
    let x: MTLBuffer, gate: MTLBuffer, up: MTLBuffer
    let gateOutput: MTLBuffer, upOutput: MTLBuffer
    let reference: MTLBuffer, candidate: MTLBuffer
    let allocatedBytes: Int

    init(device: MTLDevice, n: Int, m: Int, k: Int, extreme: Bool = false) throws {
        guard n > 0 && m > 0 && k > 0 && n <= 2048 && m <= 12288 && k <= 4096 else {
            throw ProbeError("shape outside the bounded synthetic fixture")
        }
        self.n = n; self.m = m; self.k = k
        let lengths = [n * k * 2, m * k * 2, m * k * 2, n * m * 4, n * m * 4,
                       (n * m + 2 * redzone) * 2, (n * m + 2 * redzone) * 2]
        allocatedBytes = lengths.reduce(0, +)
        guard allocatedBytes <= maximumDataBytes else { throw ProbeError("data bound exceeded before allocation") }
        func buffer(_ length: Int) throws -> MTLBuffer {
            guard let result = device.makeBuffer(length: length, options: .storageModeShared) else {
                throw ProbeError("Metal allocation failed")
            }
            return result
        }
        x = try buffer(lengths[0]); gate = try buffer(lengths[1]); self.up = try buffer(lengths[2])
        gateOutput = try buffer(lengths[3]); upOutput = try buffer(lengths[4])
        reference = try buffer(lengths[5]); candidate = try buffer(lengths[6])
        let xp = x.contents().bindMemory(to: Float16.self, capacity: n * k)
        let gp = gate.contents().bindMemory(to: Float16.self, capacity: m * k)
        let up = self.up.contents().bindMemory(to: Float16.self, capacity: m * k)
        for i in 0..<(n * k) {
            let value = Float((i * 17 + (i / k) * 11 + 19) % 257 - 128) / 128
            xp[i] = Float16(value * (extreme ? 8 : 1))
        }
        let scale: Float = 1 / sqrt(Float(k))
        for i in 0..<(m * k) {
            gp[i] = Float16(Float((i * 31 + (i / k) * 17 + 7) % 251 - 125) / 125 * scale)
            up[i] = Float16(Float((i * 43 + (i / k) * 23 + 41) % 241 - 120) / 120 * scale)
        }
        reset(reference); reset(candidate)
    }

    func reset(_ buffer: MTLBuffer) {
        let values = buffer.contents().bindMemory(to: Float16.self, capacity: n * m + 2 * redzone)
        values.initialize(repeating: poison, count: n * m + 2 * redzone)
    }

    func checkRedzones(_ buffer: MTLBuffer) throws {
        let values = buffer.contents().bindMemory(to: Float16.self, capacity: n * m + 2 * redzone)
        for i in 0..<redzone {
            guard values[i].bitPattern == poison.bitPattern &&
                    values[redzone + n * m + i].bitPattern == poison.bitPattern else {
                throw ProbeError("output redzone changed")
            }
        }
    }

    func compare(_ buffer: MTLBuffer, oracle: [Float16]?) throws -> [String: Any] {
        try checkRedzones(buffer)
        let expected = reference.contents().bindMemory(to: Float16.self, capacity: n * m + 2 * redzone)
        let actual = buffer.contents().bindMemory(to: Float16.self, capacity: n * m + 2 * redzone)
        var mismatches = 0, oracleFailures = 0
        var maximum = 0.0, oracleMaximum = 0.0
        for i in 0..<(n * m) {
            let a = actual[redzone + i], e = expected[redzone + i]
            guard a.isFinite && e.isFinite else { throw ProbeError("nonfinite or unwritten activation") }
            if a.bitPattern != e.bitPattern { mismatches += 1 }
            maximum = max(maximum, abs(Double(a) - Double(e)))
            if let oracle {
                let ref = oracle[i]
                let difference = abs(Double(a) - Double(ref))
                // f64 sums, exact half inputs, independent f64 SiLU, final half.
                // The oracle allows two half ULPs; baseline parity stays bitwise.
                let halfULP = max(abs(Double(ref.nextUp) - Double(ref)),
                                  abs(Double(ref) - Double(ref.nextDown)))
                if difference > max(0.00002, 2 * halfULP) { oracleFailures += 1 }
                oracleMaximum = max(oracleMaximum, difference)
            }
        }
        return ["values": n * m, "bit_mismatches_vs_same_tile_separate": mismatches,
                "max_abs_vs_same_tile_separate": maximum, "oracle_failures": oracleFailures,
                "max_abs_vs_f64_final_half": oracleMaximum, "f64_oracle_present": oracle != nil,
                "passed": mismatches == 0 && oracleFailures == 0, "finite": true, "redzones_unchanged": true]
    }

    func oracle() -> [Float16] {
        precondition(n * m * k <= 5_000_000)
        let xp = x.contents().bindMemory(to: Float16.self, capacity: n * k)
        let gp = gate.contents().bindMemory(to: Float16.self, capacity: m * k)
        let up = self.up.contents().bindMemory(to: Float16.self, capacity: m * k)
        var output = [Float16](repeating: 0, count: n * m)
        for row in 0..<n {
            for column in 0..<m {
                var g = 0.0, u = 0.0
                for inner in 0..<k {
                    let a = Double(xp[row * k + inner])
                    g += a * Double(gp[column * k + inner])
                    u += a * Double(up[column * k + inner])
                }
                output[row * m + column] = Float16((g / (1 + exp(-g))) * u)
            }
        }
        return output
    }
}

final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    var pipelines: [String: MTLComputePipelineState] = [:]

    init() throws {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else {
            throw ProbeError("Metal unavailable")
        }
        self.device = device; self.queue = queue
        let source = try String(contentsOf: directory.appendingPathComponent("baseline.metal"), encoding: .utf8)
            + String(contentsOf: directory.appendingPathComponent("pair.metal"), encoding: .utf8)
        let options = MTLCompileOptions()
        let library = try device.makeLibrary(source: source, options: options)
        for name in ["clef_gemm", "clef_gemm_64x128", "clef_silu_mul_to_f16",
                     "clef_ffn_up_silu_64x64", "clef_ffn_up_silu_64x128", "ffn_pair_64", "ffn_pair_128"] {
            guard let function = library.makeFunction(name: name) else { throw ProbeError("missing pipeline \(name)") }
            let pipeline = try device.makeComputePipelineState(function: function)
            guard pipeline.threadExecutionWidth == 32 && pipeline.maxTotalThreadsPerThreadgroup >= 128 else {
                throw ProbeError("unsupported execution width for \(name)")
            }
            pipelines[name] = pipeline
        }
        try emit(["kind": "device", "name": device.name, "max_threadgroup_memory": device.maxThreadgroupMemoryLength,
                  "maximum_fixture_bytes": maximumDataBytes, "conditions": "shared machine; synthetic kernels only"])
    }

    func run(_ fixture: Fixture, mode: Mode, tile: Int, output: MTLBuffer, repeats: Int = 1) throws -> Timing {
        try autoreleasepool {
            guard repeats > 0 && repeats <= 2 else { throw ProbeError("repetition bound exceeded") }
            let began = ProcessInfo.processInfo.systemUptime
            guard let command = queue.makeCommandBuffer(),
                  let encoder = command.makeComputeCommandEncoder(dispatchType: .concurrent) else {
                throw ProbeError("command allocation failed")
            }
            var shape = SIMD4<UInt32>(UInt32(fixture.n), UInt32(fixture.m), UInt32(fixture.k), 0)
            var dispatched = false
            func launch(_ name: String, _ buffers: [(MTLBuffer, Int)], shapeIndex: Int) {
                if dispatched { encoder.memoryBarrier(scope: .buffers) }
                dispatched = true
                encoder.setComputePipelineState(pipelines[name]!)
                for (index, item) in buffers.enumerated() { encoder.setBuffer(item.0, offset: item.1, index: index) }
                encoder.setBytes(&shape, length: 16, index: shapeIndex)
                encoder.dispatchThreadgroups(MTLSize(width: (fixture.m + tile - 1) / tile,
                                                     height: (fixture.n + 63) / 64, depth: 1),
                                             threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
            }
            for _ in 0..<repeats {
                if mode == .fusedPair {
                    launch("ffn_pair_\(tile)", [(fixture.x, 0), (fixture.gate, 0), (fixture.up, 0), (output, redzone * 2)], shapeIndex: 4)
                } else {
                    let gemm = tile == 64 ? "clef_gemm" : "clef_gemm_64x128"
                    launch(gemm, [(fixture.x, 0), (fixture.gate, 0), (fixture.gateOutput, 0)], shapeIndex: 3)
                    if mode == .fusedUp {
                        launch("clef_ffn_up_silu_64x\(tile)", [(fixture.x, 0), (fixture.up, 0),
                               (fixture.gateOutput, 0), (output, redzone * 2)], shapeIndex: 4)
                    } else {
                        launch(gemm, [(fixture.x, 0), (fixture.up, 0), (fixture.upOutput, 0)], shapeIndex: 3)
                        encoder.memoryBarrier(scope: .buffers)
                        encoder.setComputePipelineState(pipelines["clef_silu_mul_to_f16"]!)
                        encoder.setBuffer(fixture.gateOutput, offset: 0, index: 0)
                        encoder.setBuffer(fixture.upOutput, offset: 0, index: 1)
                        encoder.setBuffer(output, offset: redzone * 2, index: 2)
                        var count = UInt64(fixture.n * fixture.m)
                        encoder.setBytes(&count, length: 8, index: 3)
                        encoder.dispatchThreadgroups(MTLSize(width: (fixture.n * fixture.m + 127) / 128, height: 1, depth: 1),
                                                     threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
                    }
                }
            }
            encoder.endEncoding(); command.commit(); command.waitUntilCompleted()
            guard command.status == .completed else { throw ProbeError("GPU completion failed: \(String(describing: command.error))") }
            let gpu = command.gpuEndTime - command.gpuStartTime
            guard gpu.isFinite && gpu > 0 else { throw ProbeError("invalid GPU timestamps") }
            return Timing(gpu: gpu / Double(repeats), wall: (ProcessInfo.processInfo.systemUptime - began) / Double(repeats))
        }
    }
}

do {
    let runner = try Runner()
    var allPassed = true
    for (n, m, k, extreme) in [(1, 1, 1, false), (3, 31, 7, true), (7, 33, 63, false),
                               (17, 65, 64, false), (63, 127, 65, false), (64, 128, 128, false),
                               (65, 129, 257, false), (129, 131, 129, false)] {
        try autoreleasepool {
            let fixture = try Fixture(device: runner.device, n: n, m: m, k: k, extreme: extreme)
            let oracle = fixture.oracle()
            for tile in [64, 128] {
                _ = try runner.run(fixture, mode: .separate, tile: tile, output: fixture.reference)
                for mode in Mode.allCases {
                    fixture.reset(fixture.candidate)
                    _ = try runner.run(fixture, mode: mode, tile: tile, output: fixture.candidate)
                    var result = try fixture.compare(fixture.candidate, oracle: oracle)
                    result.merge(["kind": "small_correctness", "n": n, "m": m, "k": k, "tile": tile,
                                  "mode": mode.rawValue, "data_bytes": fixture.allocatedBytes]) { _, new in new }
                    allPassed = allPassed && (result["passed"] as! Bool)
                    try emit(result)
                }
            }
        }
    }
    guard allPassed else { throw ProbeError("small oracle or exact baseline parity failed; benchmark withheld") }
    if !checkOnly {
        for n in [65, 1024, 2048] {
            try autoreleasepool {
                let fixture = try Fixture(device: runner.device, n: n, m: 12288, k: 4096)
                var approved: [Int: [Mode]] = [:]
                for tile in [64, 128] {
                    _ = try runner.run(fixture, mode: .separate, tile: tile, output: fixture.reference)
                    for mode in Mode.allCases {
                        fixture.reset(fixture.candidate)
                        _ = try runner.run(fixture, mode: mode, tile: tile, output: fixture.candidate)
                        var result = try fixture.compare(fixture.candidate, oracle: nil)
                        result.merge(["kind": "real_shape_correctness", "n": n, "m": fixture.m, "k": fixture.k,
                                      "tile": tile, "mode": mode.rawValue, "data_bytes": fixture.allocatedBytes]) { _, new in new }
                        if result["passed"] as! Bool { approved[tile, default: []].append(mode) }
                        else { allPassed = false }
                        try emit(result)
                    }
                }
                for round in 0..<6 {
                    let pairs = [64, 128].flatMap { tile in (approved[tile] ?? []).map { (tile, $0) } }
                    for (tile, mode) in (round % 2 == 0 ? pairs : Array(pairs.reversed())) {
                        let timing = try runner.run(fixture, mode: mode, tile: tile, output: fixture.candidate, repeats: 2)
                        try emit(["kind": "timing", "round": round, "n": n, "m": fixture.m, "k": fixture.k,
                                  "mode": mode.rawValue, "tile": tile, "gpu_seconds_per_ffn_pair": timing.gpu,
                                  "wall_seconds_per_ffn_pair": timing.wall, "pairs_per_command": 2,
                                  "load_average": loadAverage(),
                                  "logical_intermediate_bytes_removed": mode == .fusedPair ? n * fixture.m * 16 :
                                    (mode == .fusedUp ? n * fixture.m * 8 : 0)])
                    }
                }
                // A later write must not escape either output allocation.
                try fixture.checkRedzones(fixture.reference); try fixture.checkRedzones(fixture.candidate)
            }
        }
    }
    try emit(["kind": "complete", "passed": allPassed, "model_loaded": false,
              "scope": checkOnly ? "synthetic small correctness" : "synthetic correctness and FFN pair timing; no full-model claim"])
    exit(allPassed ? 0 : 1)
} catch {
    try? emit(["kind": "failure", "error": String(describing: error), "passed": false, "model_loaded": false])
    exit(1)
}
