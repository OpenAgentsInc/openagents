// Scratch-only comparison of the frozen block16 solve and a column-local solve.
import Foundation
import Metal
import Darwin

struct Failure: Error, CustomStringConvertible {
    let description: String
    init(_ message: String) { description = message }
}
func require(_ condition: Bool, _ message: String) throws {
    if !condition { throw Failure(message) }
}
func emit(_ value: [String: Any]) throws {
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]))
    FileHandle.standardOutput.write(Data([10]))
}
func loadAverage() -> [Double] {
    var values = [Double](repeating: 0, count: 3)
    let count = values.withUnsafeMutableBufferPointer { getloadavg($0.baseAddress!, 3) }
    return count == 3 ? values : []
}
let arguments = CommandLine.arguments
try require(arguments.count == 3 && ["check", "bench"].contains(arguments[2]), "Usage: probe DIRECTORY check|bench")
let root = URL(fileURLWithPath: arguments[1])
let benchmarking = arguments[2] == "bench"
let variants = ["clef_delta_block_scan", "clef_delta_block_scan_register_columns"]
let redzone = 16, dim = 128, block = 16
let sentinel = Float(bitPattern: 0x7fc00555)
let dataBound = 512 * 1024 * 1024
let started = ProcessInfo.processInfo.systemUptime
func deadline() throws {
    try require(ProcessInfo.processInfo.systemUptime - started < 55, "internal 55-second deadline reached")
}

struct Case {
    let n: Int, kh: Int, vh: Int, reordered: Bool, width: Int, offset: Int
    var q: [Float], k: [Float], conv: [Float], decay: [Float], beta: [Float], state: [Float]
    init(_ n: Int, _ kh: Int, _ vh: Int, _ reordered: Bool, correlated: Bool = false, actual: Bool = false) {
        self.n = n; self.kh = kh; self.vh = vh; self.reordered = reordered
        offset = actual ? 2 * kh * dim : 16
        width = offset + vh * dim + (actual ? 0 : 16)
        func values(_ count: Int, _ seed: Int, _ denominator: Float) -> [Float] {
            (0..<count).map { Float(($0 * 13 + $0 / 7 + seed) % 17 - 8) / denominator }
        }
        q = values(n * kh * dim, 3, 1024)
        k = values(n * kh * dim, 11, 64)
        conv = values(n * width, 5, 16)
        decay = (0..<n * vh).map { i in
            switch i % 11 { case 0: return 0; case 1: return 1; case 2: return powf(2, -40); default: return 0.875 }
        }
        beta = (0..<n * vh).map { [Float(0), 0.25, 0.5, 1][$0 % 4] }
        state = values(vh * dim * dim, 7, 512)
        if correlated {
            for t in 0..<n {
                for h in 0..<kh {
                    let key = (0..<dim).map { i in
                        Double((i * 7 + h * 11) % 17) / 16 - 0.5 + Double((t * 3 + i * 5) % 11) / 4096
                    }
                    let norm = sqrt(key.reduce(0) { $0 + $1 * $1 })
                    for i in 0..<dim {
                        k[(t * kh + h) * dim + i] = Float(key[i] / norm)
                        q[(t * kh + h) * dim + i] = Float(key[(i + 7) % dim] / norm / sqrt(Double(dim)))
                    }
                }
                for h in 0..<vh {
                    decay[t * vh + h] = (t + h) % 7 == 0 ? 1 : 1023 / 1024
                    beta[t * vh + h] = [Float(0.75), 0.9375, 1][(t + h) % 3]
                    for v in 0..<dim { conv[t * width + offset + h * dim + v] = Float((t * 37 + h * 17 + v * 13) % 97) / 16 - 3 }
                }
            }
            for i in state.indices { state[i] *= 16 }
        }
    }
    func keyHead(_ head: Int) -> Int { reordered ? head % kh : head / (vh / kh) }
    var hostBytes: Int { (q.count + k.count + conv.count + decay.count + beta.count + state.count) * 4 }
}

// Dense f64 recurrence, ported from the independent checked-in CPU oracle.
func oracle(_ c: Case) -> ([Double], [Double]) {
    var state = c.state.map(Double.init)
    var output = [Double](repeating: 0, count: c.n * c.vh * dim)
    for t in 0..<c.n {
        for h in 0..<c.vh {
            let qk = (t * c.kh + c.keyHead(h)) * dim
            let g = Double(c.decay[t * c.vh + h]), b = Double(c.beta[t * c.vh + h])
            for v in 0..<dim {
                let s = (h * dim + v) * dim
                var predicted = 0.0
                for i in 0..<dim {
                    state[s + i] *= g
                    predicted += state[s + i] * Double(c.k[qk + i])
                }
                let correction = (Double(c.conv[t * c.width + c.offset + h * dim + v]) - predicted) * b
                var answer = 0.0
                for i in 0..<dim {
                    state[s + i] += correction * Double(c.k[qk + i])
                    answer += state[s + i] * Double(c.q[qk + i])
                }
                output[(t * c.vh + h) * dim + v] = answer
            }
        }
    }
    return (output, state)
}

final class Guarded {
    let buffer: MTLBuffer, count: Int
    init(_ device: MTLDevice, _ values: [Float]) throws {
        count = values.count
        let bytes = (count + 2 * redzone) * 4
        try require(bytes > 0 && bytes <= dataBound, "individual allocation exceeds fixture bound")
        guard let allocation = device.makeBuffer(length: bytes, options: .storageModeShared) else { throw Failure("Metal buffer allocation failed") }
        buffer = allocation
        let pointer = buffer.contents().bindMemory(to: Float.self, capacity: count + 2 * redzone)
        pointer.initialize(repeating: sentinel, count: count + 2 * redzone)
        write(values)
    }
    func write(_ values: [Float]) {
        precondition(values.count == count)
        values.withUnsafeBufferPointer { source in
            if count != 0 { buffer.contents().advanced(by: redzone * 4).copyMemory(from: source.baseAddress!, byteCount: count * 4) }
        }
    }
    func check() throws {
        let pointer = buffer.contents().bindMemory(to: Float.self, capacity: count + 2 * redzone)
        for i in 0..<redzone {
            try require(pointer[i].bitPattern == sentinel.bitPattern && pointer[redzone + count + i].bitPattern == sentinel.bitPattern, "buffer redzone changed")
        }
    }
    func read() throws -> [Float] {
        try check()
        return Array(UnsafeBufferPointer(start: buffer.contents().advanced(by: redzone * 4).assumingMemoryBound(to: Float.self), count: count))
    }
    func unchanged(_ expected: [Float]) throws {
        try check()
        try require(expected.count == count, "input comparison length mismatch")
        let pointer = buffer.contents().advanced(by: redzone * 4).assumingMemoryBound(to: Float.self)
        for i in 0..<count { try require(pointer[i].bitPattern == expected[i].bitPattern, "input mutated at \(i)") }
    }
}
struct Dimensions {
    var n: UInt32, keyHeads: UInt32, valueHeads: UInt32, reordered: UInt32, width: UInt32, offset: UInt32
}
struct Timing { let gpu: Double, wall: Double, gpuStart: Double, gpuEnd: Double }
final class Buffers {
    let q: Guarded, k: Guarded, conv: Guarded, decay: Guarded, beta: Guarded, state: Guarded, output: Guarded, workspace: Guarded
    let bytes: Int
    init(_ device: MTLDevice, _ c: Case, _ n: Int, _ first: Int, _ initial: [Float]) throws {
        let work = ((n + block - 1) / block) * c.vh * (2 * block * dim + block * block + 2 * block)
        bytes = (2 * n * c.kh * dim + n * c.width + 2 * n * c.vh + initial.count + n * c.vh * dim + work + 16 * redzone) * 4
        try require(bytes + c.hostBytes + 2 * c.n * c.vh * dim * 4 <= dataBound, "fixture allocation estimate exceeds 512 MiB")
        q = try Guarded(device, Array(c.q[first * c.kh * dim..<(first + n) * c.kh * dim]))
        k = try Guarded(device, Array(c.k[first * c.kh * dim..<(first + n) * c.kh * dim]))
        conv = try Guarded(device, Array(c.conv[first * c.width..<(first + n) * c.width]))
        decay = try Guarded(device, Array(c.decay[first * c.vh..<(first + n) * c.vh]))
        beta = try Guarded(device, Array(c.beta[first * c.vh..<(first + n) * c.vh]))
        state = try Guarded(device, initial)
        output = try Guarded(device, [Float](repeating: sentinel, count: n * c.vh * dim))
        workspace = try Guarded(device, [Float](repeating: sentinel, count: work))
    }
    func checkInputs(_ c: Case, _ n: Int, _ first: Int) throws {
        try q.unchanged(Array(c.q[first * c.kh * dim..<(first + n) * c.kh * dim]))
        try k.unchanged(Array(c.k[first * c.kh * dim..<(first + n) * c.kh * dim]))
        try conv.unchanged(Array(c.conv[first * c.width..<(first + n) * c.width]))
        try decay.unchanged(Array(c.decay[first * c.vh..<(first + n) * c.vh]))
        try beta.unchanged(Array(c.beta[first * c.vh..<(first + n) * c.vh]))
        try state.check(); try output.check(); try workspace.check()
    }
}
final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    var pipelines = [String: MTLComputePipelineState]()
    init() throws {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { throw Failure("Metal unavailable") }
        self.device = device; self.queue = queue
        let source = try String(contentsOf: root.appendingPathComponent("kernels.metal"), encoding: .utf8)
        let library = try device.makeLibrary(source: source, options: MTLCompileOptions())
        for name in ["clef_delta_block_prepare"] + variants {
            guard let function = library.makeFunction(name: name) else { throw Failure("kernel absent: \(name)") }
            let pipeline = try device.makeComputePipelineState(function: function)
            try require(pipeline.threadExecutionWidth == 32 && pipeline.maxTotalThreadsPerThreadgroup >= 128 && pipeline.staticThreadgroupMemoryLength <= device.maxThreadgroupMemoryLength, "unsupported pipeline dimensions")
            pipelines[name] = pipeline
            try emit(["kind": "pipeline", "name": name, "thread_width": pipeline.threadExecutionWidth, "max_threads": pipeline.maxTotalThreadsPerThreadgroup, "static_threadgroup_bytes": pipeline.staticThreadgroupMemoryLength])
        }
        try require(MemoryLayout<Dimensions>.size == 24, "shader argument layout differs")
        try emit(["kind": "device", "name": device.name, "data_bound_bytes": dataBound, "load_average": loadAverage(), "conditions": "shared machine; synthetic inputs; no model execution"])
    }
    func execute(_ variant: String, _ c: Case, _ n: Int, _ buffers: Buffers, prepare: Bool = true) throws -> Timing {
        try deadline()
        return try autoreleasepool {
            let began = ProcessInfo.processInfo.systemUptime
            guard let command = queue.makeCommandBuffer(), let encoder = command.makeComputeCommandEncoder(dispatchType: .concurrent) else { throw Failure("command allocation failed") }
            var args = Dimensions(n: UInt32(n), keyHeads: UInt32(c.kh), valueHeads: UInt32(c.vh), reordered: c.reordered ? 1 : 0, width: UInt32(c.width), offset: UInt32(c.offset))
            if prepare {
                encoder.setComputePipelineState(pipelines["clef_delta_block_prepare"]!)
                for (index, buffer) in [buffers.q, buffers.k, buffers.conv, buffers.decay, buffers.beta, buffers.workspace].enumerated() { encoder.setBuffer(buffer.buffer, offset: redzone * 4, index: index) }
                encoder.setBytes(&args, length: 24, index: 6)
                encoder.dispatchThreadgroups(MTLSize(width: (n + block - 1) / block, height: c.vh, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
                encoder.memoryBarrier(scope: .buffers)
            }
            encoder.setComputePipelineState(pipelines[variant]!)
            for (index, buffer) in [buffers.q, buffers.k, buffers.workspace, buffers.state, buffers.output].enumerated() { encoder.setBuffer(buffer.buffer, offset: redzone * 4, index: index) }
            encoder.setBytes(&args, length: 24, index: 5)
            encoder.dispatchThreadgroups(MTLSize(width: 4, height: c.vh, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
            encoder.endEncoding(); command.commit(); command.waitUntilCompleted()
            try require(command.status == .completed, "GPU command failed: \(String(describing: command.error))")
            let gpuStart = command.gpuStartTime, gpuEnd = command.gpuEndTime
            try require(gpuStart.isFinite && gpuEnd.isFinite && gpuEnd > gpuStart, "invalid GPU timestamps")
            return Timing(gpu: gpuEnd - gpuStart, wall: ProcessInfo.processInfo.systemUptime - began, gpuStart: gpuStart, gpuEnd: gpuEnd)
        }
    }
    func run(_ variant: String, _ c: Case, _ chunks: [Int]) throws -> ([Float], [Float]) {
        try require(chunks.reduce(0, +) == c.n && chunks.allSatisfy({ $0 > 0 }), "invalid chunk schedule")
        var state = c.state, output = [Float](), first = 0
        output.reserveCapacity(c.n * c.vh * dim)
        for n in chunks {
            try autoreleasepool {
                let buffers = try Buffers(device, c, n, first, state)
                _ = try execute(variant, c, n, buffers)
                try buffers.checkInputs(c, n, first)
                output.append(contentsOf: try buffers.output.read())
                state = try buffers.state.read()
            }
            first += n
        }
        return (output, state)
    }
}
func exact(_ actual: [Float], _ expected: [Float]) throws -> [String: Any] {
    try require(actual.count == expected.count, "comparison length mismatch")
    var mismatches = 0, maximum = 0.0
    for i in actual.indices {
        try require(actual[i].isFinite && expected[i].isFinite, "nonfinite or unwritten result at \(i)")
        if actual[i].bitPattern != expected[i].bitPattern { mismatches += 1 }
        maximum = max(maximum, abs(Double(actual[i]) - Double(expected[i])))
    }
    return ["values": actual.count, "bit_mismatches": mismatches, "max_abs": maximum, "passed": mismatches == 0]
}
func oracleError(_ actual: [Float], _ expected: [Double]) throws -> [String: Any] {
    try require(actual.count == expected.count, "oracle length mismatch")
    var maximum = 0.0, failures = 0
    for i in actual.indices {
        try require(actual[i].isFinite && expected[i].isFinite, "oracle result nonfinite")
        let error = abs(Double(actual[i]) - expected[i])
        maximum = max(maximum, error)
        if error > 0.00002 + 0.0002 * abs(expected[i]) { failures += 1 }
    }
    return ["values": actual.count, "max_abs": maximum, "failed_values": failures, "passed": failures == 0]
}
func check(_ runner: Runner, _ c: Case, _ chunks: [Int], _ label: String, _ expected: ([Double], [Double])) throws {
    let baseline = try runner.run(variants[0], c, chunks)
    let candidate = try runner.run(variants[1], c, chunks)
    let outputs = try exact(candidate.0, baseline.0), states = try exact(candidate.1, baseline.1)
    let baseOutput = try oracleError(baseline.0, expected.0), baseState = try oracleError(baseline.1, expected.1)
    let candOutput = try oracleError(candidate.0, expected.0), candState = try oracleError(candidate.1, expected.1)
    let passed = [outputs, states, baseOutput, baseState, candOutput, candState].allSatisfy { $0["passed"] as? Bool == true }
    try emit(["kind": "correctness", "case": label, "n": c.n, "key_heads": c.kh, "value_heads": c.vh, "reordered": c.reordered, "chunks": chunks, "output": outputs, "state": states, "baseline_f64_output": baseOutput, "baseline_f64_state": baseState, "candidate_f64_output": candOutput, "candidate_f64_state": candState, "redzones_unchanged": true, "inputs_unchanged": true, "passed": passed])
    try require(passed, "correctness gate failed for \(label)")
}

do {
    let runner = try Runner()
    let tails = [(1,1,1,false),(15,1,1,false),(16,2,4,true),(17,2,4,false),(31,2,4,false),(32,2,4,true),(33,2,4,false),(65,2,4,true)]
    for (n, kh, vh, reordered) in tails {
        try autoreleasepool {
            let c = Case(n, kh, vh, reordered), expected = oracle(c)
            try check(runner, c, [n], "tail", expected)
        }
    }
    for reordered in [false, true] {
        try autoreleasepool {
            var c = Case(65, 2, 4, reordered)
            c.decay = [Float](repeating: 1, count: c.n * c.vh)
            c.decay[15 * c.vh] = 0; c.decay[16 * c.vh + 1] = 0
            for t in 0..<c.n { c.decay[t * c.vh + 2] = powf(2, -40) }
            let expected = oracle(c)
            for chunks in [[65], [33,32], [15,17,31,2], [Int](repeating: 1, count: 65)] { try check(runner, c, chunks, "zero-decay", expected) }
        }
        try autoreleasepool {
            let c = Case(155, 2, 4, reordered, correlated: true), expected = oracle(c)
            for chunks in [[155], [31,33,64,27]] { try check(runner, c, chunks, "correlated", expected) }
        }
    }
    try autoreleasepool {
        let c = Case(2032, 1, 1, false, correlated: true), expected = oracle(c)
        for chunks in [[2032], [257,257,257,257,257,257,257,233]] { try check(runner, c, chunks, "long-correlated", expected) }
    }
    if benchmarking {
        for n in [1024, 2048] {
            try autoreleasepool {
                let c = Case(n, 16, 32, false, correlated: true, actual: true)
                let buffers = try Buffers(runner.device, c, n, 0, c.state)
                _ = try runner.execute(variants[0], c, n, buffers)
                let expectedOutput = try buffers.output.read(), expectedState = try buffers.state.read()
                buffers.state.write(c.state)
                buffers.output.write([Float](repeating: sentinel, count: n * c.vh * dim))
                _ = try runner.execute(variants[1], c, n, buffers)
                let output = try exact(buffers.output.read(), expectedOutput), state = try exact(buffers.state.read(), expectedState)
                try buffers.checkInputs(c, n, 0)
                let passed = output["passed"] as? Bool == true && state["passed"] as? Bool == true
                try emit(["kind": "actual_shape_correctness", "n": n, "key_heads": 16, "value_heads": 32, "conv_width": c.width, "value_offset": c.offset, "output": output, "state": state, "fixture_device_bytes": buffers.bytes, "fixture_host_bytes": c.hostBytes, "passed": passed, "inputs": "deterministic correlated normalized values, not model activations", "inputs_unchanged": true, "redzones_unchanged": true])
                try require(passed, "actual-shape bitwise gate failed")
                for scope in ["prepare-and-scan", "scan-only"] {
                    for round in 0..<6 {
                        let order = round % 2 == 0 ? variants : Array(variants.reversed())
                        for variant in order {
                            buffers.state.write(c.state)
                            let time = try runner.execute(variant, c, n, buffers, prepare: scope == "prepare-and-scan")
                            try emit(["kind": "timing", "scope": scope, "variant": variant, "n": n, "round": round, "gpu_ms": time.gpu * 1000, "wall_ms": time.wall * 1000, "gpu_start_seconds": time.gpuStart, "gpu_end_seconds": time.gpuEnd, "load_average": loadAverage()])
                        }
                    }
                }
                try buffers.checkInputs(c, n, 0)
                let finalOutput = try exact(buffers.output.read(), expectedOutput), finalState = try exact(buffers.state.read(), expectedState)
                try require(finalOutput["passed"] as? Bool == true && finalState["passed"] as? Bool == true, "timing run changed numerical result")
            }
        }
    }
    try emit(["kind": "complete", "correctness_passed": true, "timing_included": benchmarking, "model_executed": false, "elapsed_seconds": ProcessInfo.processInfo.systemUptime - started])
} catch {
    try? emit(["kind": "failure", "error": String(describing: error)])
    exit(1)
}
