// Scratch-only sequential/block32/prepared-diagonal comparison. No model loading.
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
let variants = ["sequential", "block32", "diagonal32"]
let redzone = 16, dim = 128, block = 32
let sentinel = Float(bitPattern: 0x7fc00555)
let dataBound = 768 * 1024 * 1024
let started = ProcessInfo.processInfo.systemUptime
func deadline() throws {
    try require(ProcessInfo.processInfo.systemUptime - started < 55, "internal 55-second deadline reached")
}

struct Case {
    let n: Int, kh: Int, vh: Int, reordered: Bool, width: Int, offset: Int
    var q: [Float], k: [Float], conv: [Float], decay: [Float], beta: [Float], state: [Float], kq: [Float]
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
        kq = [Float](repeating: 0, count: n * kh)
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
    var hostBytes: Int { (q.count + k.count + conv.count + decay.count + beta.count + state.count + kq.count) * 4 }
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
    let q: Guarded, k: Guarded, conv: Guarded, decay: Guarded, beta: Guarded, kq: Guarded, state: Guarded, output: Guarded, workspace: Guarded
    let bytes: Int
    init(_ device: MTLDevice, _ c: Case, _ n: Int, _ first: Int, _ initial: [Float]) throws {
        let work = ((n + block - 1) / block) * c.vh * (2 * block * dim + block * block + 2 * block)
        bytes = (2 * n * c.kh * dim + n * c.width + 2 * n * c.vh + initial.count + n * c.vh * dim + work + n * c.kh + 18 * redzone) * 4
        try require(bytes + c.hostBytes + 6 * c.n * c.vh * dim * 4 + 6 * initial.count * 4 <= dataBound, "fixture allocation estimate exceeds 768 MiB")
        q = try Guarded(device, Array(c.q[first * c.kh * dim..<(first + n) * c.kh * dim]))
        k = try Guarded(device, Array(c.k[first * c.kh * dim..<(first + n) * c.kh * dim]))
        conv = try Guarded(device, Array(c.conv[first * c.width..<(first + n) * c.width]))
        decay = try Guarded(device, Array(c.decay[first * c.vh..<(first + n) * c.vh]))
        beta = try Guarded(device, Array(c.beta[first * c.vh..<(first + n) * c.vh]))
        kq = try Guarded(device, Array(c.kq[first * c.kh..<(first + n) * c.kh]))
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
        try kq.unchanged(Array(c.kq[first * c.kh..<(first + n) * c.kh]))
        try state.check(); try output.check(); try workspace.check()
    }
}
final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    var pipelines = [String: MTLComputePipelineState]()
    init() throws {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { throw Failure("Metal unavailable") }
        self.device = device; self.queue = queue
        for (file, entries) in [
            ("baseline.metal", [("block32-prepare", "clef_delta_block32_prepare", 128), ("block32", "clef_delta_block32_scan", 128)]),
            ("candidate.metal", [("diagonal32-prepare", "clef_delta_block32_prepare", 128), ("diagonal32", "clef_delta_block32_scan", 128)]),
            ("support.metal", [("sequential", "clef_delta_scan128_256x2", 256), ("dot", "probe_prepared_dot", 128), ("prep", "clef_delta_prep", 128)]),
        ] {
            let source = try String(contentsOf: root.appendingPathComponent(file), encoding: .utf8)
            let library = try device.makeLibrary(source: source, options: MTLCompileOptions())
            for (label, functionName, threads) in entries {
                guard let function = library.makeFunction(name: functionName) else { throw Failure("kernel absent: \(functionName)") }
                let pipeline = try device.makeComputePipelineState(function: function)
                try require(pipeline.threadExecutionWidth == 32 && pipeline.maxTotalThreadsPerThreadgroup >= threads && pipeline.staticThreadgroupMemoryLength <= device.maxThreadgroupMemoryLength, "unsupported pipeline dimensions")
                pipelines[label] = pipeline
                try emit(["kind": "pipeline", "label": label, "function": functionName, "source": file, "thread_width": pipeline.threadExecutionWidth, "max_threads": pipeline.maxTotalThreadsPerThreadgroup, "static_threadgroup_bytes": pipeline.staticThreadgroupMemoryLength])
            }
        }
        try require(MemoryLayout<Dimensions>.size == 24, "shader argument layout differs")
        try emit(["kind": "device", "name": device.name, "data_bound_bytes": dataBound, "load_average": loadAverage(), "conditions": "shared machine; synthetic inputs; no model execution"])
    }
    func finish(_ command: MTLCommandBuffer, _ encoder: MTLComputeCommandEncoder, _ began: Double) throws -> Timing {
        encoder.endEncoding(); command.commit(); command.waitUntilCompleted()
        try require(command.status == .completed, "GPU command failed: \(String(describing: command.error))")
        let first = command.gpuStartTime, last = command.gpuEndTime
        try require(first.isFinite && last.isFinite && last > first, "invalid GPU timestamps")
        return Timing(gpu: last - first, wall: ProcessInfo.processInfo.systemUptime - began, gpuStart: first, gpuEnd: last)
    }
    func prepared(_ source: Case, fromConvolution: Bool = false) throws -> Case {
        try deadline()
        return try autoreleasepool {
            var c = source
            let q = try Guarded(device, c.q), k = try Guarded(device, c.k)
            let kq = try Guarded(device, [Float](repeating: sentinel, count: c.n * c.kh))
            guard let command = queue.makeCommandBuffer(), let encoder = command.makeComputeCommandEncoder() else { throw Failure("dot command allocation failed") }
            let began = ProcessInfo.processInfo.systemUptime
            if fromConvolution {
                try require(c.offset == 2 * c.kh * dim && c.width == (2 * c.kh + c.vh) * dim, "production prep requires native convolution layout")
                let conv = try Guarded(device, c.conv)
                let zeros = [Float](repeating: 0, count: c.n * c.vh)
                let alpha = try Guarded(device, zeros), betaIn = try Guarded(device, zeros)
                let weights = [Float](repeating: -1, count: c.vh), dtValues = [Float](repeating: 0, count: c.vh)
                let a = try Guarded(device, weights), dt = try Guarded(device, dtValues)
                let decay = try Guarded(device, [Float](repeating: sentinel, count: c.n * c.vh))
                let beta = try Guarded(device, [Float](repeating: sentinel, count: c.n * c.vh))
                var args = [UInt32(c.kh), UInt32(c.vh), UInt32(dim), UInt32(c.width)]
                encoder.setComputePipelineState(pipelines["prep"]!)
                for (index, buffer) in [conv, alpha, betaIn, a, dt, q, k, decay, beta, kq].enumerated() { encoder.setBuffer(buffer.buffer, offset: redzone * 4, index: index) }
                args.withUnsafeBytes { encoder.setBytes($0.baseAddress!, length: $0.count, index: 10) }
                encoder.dispatchThreadgroups(MTLSize(width: c.n, height: c.kh, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
                _ = try finish(command, encoder, began)
                c.q = try q.read(); c.k = try k.read(); c.decay = try decay.read(); c.beta = try beta.read()
                try conv.unchanged(c.conv); try alpha.unchanged(zeros); try betaIn.unchanged(zeros); try a.unchanged(weights); try dt.unchanged(dtValues)
            } else {
                var args = SIMD2<UInt32>(UInt32(c.n), UInt32(c.kh))
                encoder.setComputePipelineState(pipelines["dot"]!)
                for (index, buffer) in [q, k, kq].enumerated() { encoder.setBuffer(buffer.buffer, offset: redzone * 4, index: index) }
                encoder.setBytes(&args, length: 8, index: 3)
                encoder.dispatchThreadgroups(MTLSize(width: c.n, height: c.kh, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
                _ = try finish(command, encoder, began)
                try q.unchanged(c.q); try k.unchanged(c.k)
            }
            c.kq = try kq.read()
            try require(c.kq.allSatisfy(\.isFinite), "prepared diagonal contains nonfinite values")
            return c
        }
    }
    func execute(_ variant: String, _ c: Case, _ n: Int, _ buffers: Buffers) throws -> Timing {
        try deadline()
        return try autoreleasepool {
            let began = ProcessInfo.processInfo.systemUptime
            guard let command = queue.makeCommandBuffer(), let encoder = command.makeComputeCommandEncoder(dispatchType: .concurrent) else { throw Failure("command allocation failed") }
            var args = Dimensions(n: UInt32(n), keyHeads: UInt32(c.kh), valueHeads: UInt32(c.vh), reordered: c.reordered ? 1 : 0, width: UInt32(c.width), offset: UInt32(c.offset))
            if variant == "sequential" {
                encoder.setComputePipelineState(pipelines[variant]!)
                for (index, buffer) in [buffers.q, buffers.k, buffers.conv, buffers.decay, buffers.beta, buffers.kq, buffers.state, buffers.output].enumerated() { encoder.setBuffer(buffer.buffer, offset: redzone * 4, index: index) }
                encoder.setBytes(&args, length: 24, index: 8)
                encoder.dispatchThreadgroups(MTLSize(width: c.vh * dim / 64, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 256, height: 1, depth: 1))
            } else {
                encoder.setComputePipelineState(pipelines[variant + "-prepare"]!)
                for (index, buffer) in [buffers.q, buffers.k, buffers.conv, buffers.decay, buffers.beta, buffers.workspace].enumerated() { encoder.setBuffer(buffer.buffer, offset: redzone * 4, index: index) }
                if variant == "diagonal32" {
                    encoder.setBuffer(buffers.kq.buffer, offset: redzone * 4, index: 6)
                    encoder.setBytes(&args, length: 24, index: 7)
                } else {
                    encoder.setBytes(&args, length: 24, index: 6)
                }
                encoder.dispatchThreadgroups(MTLSize(width: (n + block - 1) / block, height: c.vh, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
                encoder.memoryBarrier(scope: .buffers)
                encoder.setComputePipelineState(pipelines[variant]!)
                for (index, buffer) in [buffers.q, buffers.k, buffers.workspace, buffers.state, buffers.output].enumerated() { encoder.setBuffer(buffer.buffer, offset: redzone * 4, index: index) }
                encoder.setBytes(&args, length: 24, index: 5)
                encoder.dispatchThreadgroups(MTLSize(width: 4, height: c.vh, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1))
            }
            return try finish(command, encoder, began)
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
func closeFloat(_ actual: [Float], _ expected: [Float]) throws -> [String: Any] {
    try require(actual.count == expected.count, "comparison length mismatch")
    var maximum = 0.0, failures = 0
    for i in actual.indices {
        try require(actual[i].isFinite && expected[i].isFinite, "nonfinite or unwritten comparison")
        let error = abs(Double(actual[i]) - Double(expected[i]))
        maximum = max(maximum, error)
        if error > 0.00002 + 0.0002 * abs(Double(expected[i])) { failures += 1 }
    }
    return ["values": actual.count, "max_abs": maximum, "failed_values": failures, "passed": failures == 0]
}
func passed(_ value: [String: Any]) -> Bool { value["passed"] as? Bool == true }
func check(_ runner: Runner, _ c: Case, _ chunks: [Int], _ label: String, _ expected: ([Double], [Double]), exactDiagonal: Bool = false) throws {
    let sequential = try runner.run("sequential", c, chunks)
    let baseline = try runner.run("block32", c, chunks)
    let candidate = try runner.run("diagonal32", c, chunks)
    let states = try exact(candidate.1, baseline.1)
    let outputChange = try exact(candidate.0, baseline.0)
    let seqOutput = try closeFloat(candidate.0, sequential.0), seqState = try closeFloat(candidate.1, sequential.1)
    let diagonal = try exact(candidate.0, sequential.0)
    let checks = try [oracleError(sequential.0, expected.0), oracleError(sequential.1, expected.1), oracleError(baseline.0, expected.0), oracleError(baseline.1, expected.1), oracleError(candidate.0, expected.0), oracleError(candidate.1, expected.1)]
    let good = [states, seqOutput, seqState].allSatisfy(passed) && checks.allSatisfy(passed) && (!exactDiagonal || passed(diagonal))
    try emit(["kind": "correctness", "case": label, "n": c.n, "key_heads": c.kh, "value_heads": c.vh, "reordered": c.reordered, "chunks": chunks, "state_candidate_vs_block": states, "output_candidate_vs_block_diagnostic": outputChange, "output_candidate_vs_sequential": seqOutput, "state_candidate_vs_sequential": seqState, "prepared_diagonal_exact_gate": exactDiagonal, "candidate_vs_sequential_bitwise_diagnostic": diagonal, "f64_checks_order": ["sequential_output", "sequential_state", "block_output", "block_state", "candidate_output", "candidate_state"], "f64_checks": checks, "redzones_unchanged": true, "inputs_unchanged": true, "passed": good])
    try require(good, "correctness gate failed for \(label)")
}
do {
    let runner = try Runner()
    for reordered in [false, true] {
        var original = Case(1, 2, 6, reordered, actual: true)
        original.state = [Float](repeating: 0, count: original.state.count)
        let c = try runner.prepared(original, fromConvolution: true)
        try require(c.kq.allSatisfy({ $0 != 0 }) && c.kq[0].bitPattern != c.kq[1].bitPattern, "prepared diagonal fixture is degenerate")
        try check(runner, c, [1], "production-prepared-diagonal", oracle(c), exactDiagonal: true)
    }
    for (n, kh, vh, reordered) in [(1,1,1,false),(15,1,1,false),(31,2,6,false),(32,2,6,true),(33,2,6,false),(65,2,6,true)] {
        try autoreleasepool {
            let c = try runner.prepared(Case(n, kh, vh, reordered))
            try check(runner, c, [n], "tail", oracle(c))
        }
    }
    for reordered in [false, true] {
        try autoreleasepool {
            var original = Case(65, 2, 6, reordered)
            original.decay = [Float](repeating: 1, count: original.n * original.vh)
            original.decay[31 * original.vh] = 0; original.decay[32 * original.vh + 1] = 0
            for t in 0..<original.n { original.decay[t * original.vh + 2] = powf(2, -40) }
            let c = try runner.prepared(original), expected = oracle(c)
            for chunks in [[65], [33,32], [15,17,31,2], [Int](repeating: 1, count: 65)] { try check(runner, c, chunks, "zero-decay", expected) }
        }
        try autoreleasepool {
            let c = try runner.prepared(Case(155, 2, 6, reordered, correlated: true)), expected = oracle(c)
            for chunks in [[155], [31,33,64,27]] { try check(runner, c, chunks, "correlated", expected) }
        }
    }
    try autoreleasepool {
        let c = try runner.prepared(Case(2032, 1, 1, false, correlated: true)), expected = oracle(c)
        for chunks in [[2032], [257,257,257,257,257,257,257,233]] { try check(runner, c, chunks, "long-correlated", expected) }
    }
    if benchmarking {
        for vh in [32, 48] {
            for n in [1024, 2048] {
                try autoreleasepool {
                    let c = try runner.prepared(Case(n, 16, vh, false, correlated: true, actual: true))
                    let buffers = try Buffers(runner.device, c, n, 0, c.state)
                    var outputs = [[Float]](), states = [[Float]]()
                    for variant in variants {
                        buffers.state.write(c.state)
                        buffers.output.write([Float](repeating: sentinel, count: n * c.vh * dim))
                        _ = try runner.execute(variant, c, n, buffers)
                        outputs.append(try buffers.output.read()); states.append(try buffers.state.read())
                        try buffers.checkInputs(c, n, 0)
                    }
                    let stateUnchanged = try exact(states[2], states[1])
                    let candidateOutput = try closeFloat(outputs[2], outputs[0]), candidateState = try closeFloat(states[2], states[0])
                    let baselineOutput = try closeFloat(outputs[1], outputs[0]), baselineState = try closeFloat(states[1], states[0])
                    let good = [stateUnchanged, candidateOutput, candidateState, baselineOutput, baselineState].allSatisfy(passed)
                    try emit(["kind": "actual_shape_correctness", "model_shape": vh == 32 ? "Flash" : "27B", "n": n, "key_heads": 16, "value_heads": vh, "conv_width": c.width, "value_offset": c.offset, "candidate_vs_block_state_bitwise": stateUnchanged, "candidate_vs_sequential_output": candidateOutput, "candidate_vs_sequential_state": candidateState, "block_vs_sequential_output": baselineOutput, "block_vs_sequential_state": baselineState, "fixture_device_bytes": buffers.bytes, "fixture_host_bytes": c.hostBytes, "passed": good, "inputs": "synthetic correlated values; not captured model activations", "independent_f64": "small and long-carry fixtures only", "inputs_unchanged": true, "redzones_unchanged": true])
                    try require(good, "actual-shape correctness gate failed")
                    for variant in variants { buffers.state.write(c.state); _ = try runner.execute(variant, c, n, buffers) }
                    for round in 0..<6 {
                        for slot in 0..<variants.count {
                            let variant = variants[(round + slot) % variants.count]
                            buffers.state.write(c.state)
                            let time = try runner.execute(variant, c, n, buffers)
                            try emit(["kind": "timing", "scope": "scan-plus-block-preparation", "shared_delta_prep_excluded": true, "variant": variant, "model_shape": vh == 32 ? "Flash" : "27B", "n": n, "value_heads": vh, "round": round, "slot": slot, "gpu_ms": time.gpu * 1000, "wall_ms": time.wall * 1000, "gpu_start_seconds": time.gpuStart, "gpu_end_seconds": time.gpuEnd, "load_average": loadAverage()])
                        }
                    }
                    try buffers.checkInputs(c, n, 0)
                    let lastVariant = variants[(5 + variants.count - 1) % variants.count]
                    let lastIndex = variants.firstIndex(of: lastVariant)!
                    let finalOutput = try exact(buffers.output.read(), outputs[lastIndex]), finalState = try exact(buffers.state.read(), states[lastIndex])
                    try require(passed(finalOutput) && passed(finalState), "timing run changed numerical result")
                }
            }
        }
    }
    try emit(["kind": "complete", "correctness_passed": true, "timing_included": benchmarking, "model_executed": false, "elapsed_seconds": ProcessInfo.processInfo.systemUptime - started])
} catch {
    try? emit(["kind": "failure", "error": String(describing: error)])
    exit(1)
}
