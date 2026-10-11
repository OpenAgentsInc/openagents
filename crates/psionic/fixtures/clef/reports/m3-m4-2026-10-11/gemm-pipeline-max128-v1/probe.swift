// Pipeline-limit experiment. Both variants use identical kernels and barriers.
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
func emit(_ value: [String: Any]) throws {
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]))
    FileHandle.standardOutput.write(Data([10]))
}
func sha(_ bytes: Data) -> String { SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined() }
let args = CommandLine.arguments
try require(args.count == 3, "Usage: probe DIRECTORY ASSETS")
let root = URL(fileURLWithPath: args[1]), assets = URL(fileURLWithPath: args[2])
let start = ProcessInfo.processInfo.systemUptime
let pad = 256, explicitLimit = 1024 * 1024 * 1024, combinedLimit = 1536 * 1024 * 1024
let hidden = 4096, wide = 12288
var liveBytes = 0, peakBytes = 0, ownedCPUBytes = 0, transientBytes = 0, peakCombined = 0
var commands = 0, dispatches = 0, timingRows = 0
func deadline() throws { try require(ProcessInfo.processInfo.systemUptime - start < 50, "internal 50-second deadline reached") }
func memory(_ device: MTLDevice) throws {
    let bytes = device.currentAllocatedSize + ownedCPUBytes + transientBytes
    peakCombined = max(peakCombined, bytes)
    try require(bytes <= combinedLimit, "driver and owned CPU copies exceed 1.5 GiB")
}
final class Buffer {
    let metal: MTLBuffer, bytes: Int
    var pointer: UnsafeMutableRawPointer { metal.contents().advanced(by: pad) }
    init(_ device: MTLDevice, _ bytes: Int) throws {
        try require(bytes > 0 && bytes <= explicitLimit - 2 * pad, "invalid buffer length")
        try require(liveBytes + bytes + 2 * pad <= explicitLimit, "explicit buffers exceed 1 GiB")
        try require(device.currentAllocatedSize + ownedCPUBytes + transientBytes + bytes + 2 * pad <= combinedLimit, "allocation exceeds combined bound")
        guard let b = device.makeBuffer(length: bytes + 2 * pad, options: [.storageModeShared, .hazardTrackingModeTracked]) else { throw Failure("buffer allocation failed") }
        metal = b; self.bytes = bytes; liveBytes += bytes + 2 * pad; peakBytes = max(peakBytes, liveBytes)
        memset(b.contents(), 0xa5, bytes + 2 * pad); try memory(device)
    }
    deinit { liveBytes -= bytes + 2 * pad }
    func guards() throws {
        let p = metal.contents().assumingMemoryBound(to: UInt8.self)
        for i in 0..<pad { try require(p[i] == 0xa5 && p[pad + bytes + i] == 0xa5, "redzone changed") }
    }
    func digest() -> String { sha(Data(bytesNoCopy: metal.contents(), count: bytes + 2 * pad, deallocator: .none)) }
    func poison32() { pointer.assumingMemoryBound(to: Float.self).initialize(repeating: Float(bitPattern: 0x7fc00555), count: bytes / 4) }
    func poison16() { pointer.assumingMemoryBound(to: UInt16.self).initialize(repeating: 0x7e55, count: bytes / 2) }
    func finite(_ half: Bool) throws {
        if half {
            let p = pointer.assumingMemoryBound(to: Float16.self)
            for i in 0..<(bytes / 2) { try require(p[i].isFinite, "nonfinite or unwritten half output") }
        } else {
            let p = pointer.assumingMemoryBound(to: Float.self)
            for i in 0..<(bytes / 4) { try require(p[i].isFinite, "nonfinite or unwritten float output") }
        }
    }
}
final class Weights {
    let gate, up, down, norm: Buffer
    let before: [String]
    var buffers: [Buffer] { [gate, up, down, norm] }
    init(_ device: MTLDevice) throws {
        let doc = try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("weights.json"))) as! [String: Any]
        let entries = doc["tensors"] as! [[String: Any]]
        func read(_ filename: String) throws -> Buffer {
            try autoreleasepool {
                let entry = entries.first { $0["half_file"] as? String == filename }!
                transientBytes += hidden * wide * 2; defer { transientBytes -= hidden * wide * 2 }
                try memory(device)
                let data = try Data(contentsOf: assets.appendingPathComponent(filename), options: .mappedIfSafe)
                try require(data.count == hidden * wide * 2 && sha(data) == entry["half_sha256"] as! String, "real weight size or hash differs")
                let buffer = try Buffer(device, data.count)
                data.withUnsafeBytes { buffer.pointer.copyMemory(from: $0.baseAddress!, byteCount: data.count) }
                return buffer
            }
        }
        gate = try read("gate.f16"); up = try read("up.f16"); down = try read("down.f16")
        norm = try Buffer(device, hidden * 4)
        let p = norm.pointer.assumingMemoryBound(to: Float.self)
        for i in 0..<hidden { p[i] = 0.9 + Float(i % 17) * 0.01 }
        before = [gate, up, down, norm].map { $0.digest() }
        try emit(["kind": "weights", "manifest": doc, "guarded_source_sha256": before, "norm_weights": "deterministic synthetic f32 scale; not model norm weights", "q4_conversion_executed": false])
    }
    func unchanged() throws {
        for (i, b) in buffers.enumerated() { try b.guards(); try require(b.digest() == before[i], "immutable weights changed") }
    }
}
final class State {
    let n: Int, x, act16, gate, up: Buffer
    let input: Data, suffix: Data, copyBytes: Int, inputSHA: String, suffixSHA: String
    var all: [Buffer] { [x, act16, gate, up] }
    var outputs: [(String, Buffer, Bool)] { [("gate",gate,false),("up",up,false),("activation",act16,true),("residual",x,false)] }
    init(_ device: MTLDevice, _ n: Int) throws {
        try require([1,17,63,64,65,512,1024,2048].contains(n), "unplanned token count")
        self.n = n; x = try Buffer(device, n * hidden * 4); act16 = try Buffer(device, n * wide * 2)
        gate = try Buffer(device, n * wide * 4); up = try Buffer(device, n * wide * 4)
        let bytes = x.bytes + n * (wide - hidden) * 2
        try require(device.currentAllocatedSize + ownedCPUBytes + transientBytes + bytes <= combinedLimit, "immutable reset copies exceed bound")
        let p = x.pointer.assumingMemoryBound(to: Float.self)
        for i in 0..<(n * hidden) { p[i] = Float((i * 31 + i / hidden * 17 + n) % 251 - 125) / 125 }
        act16.poison16()
        input = Data(bytes:x.pointer,count:x.bytes)
        suffix = Data(bytes:act16.pointer.advanced(by:n*hidden*2),count:n*(wide-hidden)*2)
        copyBytes = bytes; inputSHA = sha(input); suffixSHA = sha(suffix)
        ownedCPUBytes += bytes; try memory(device)
    }
    deinit { ownedCPUBytes -= copyBytes }
    func reset() {
        input.withUnsafeBytes { x.pointer.copyMemory(from:$0.baseAddress!,byteCount:input.count) }
        act16.poison16(); gate.poison32(); up.poison32()
    }
    func guards() throws { for b in all { try b.guards() } }
    func originalInput() throws {
        try require(input.withUnsafeBytes { memcmp($0.baseAddress!,x.pointer,input.count)==0 }, "pair scope changed residual input")
        try require(suffix.withUnsafeBytes { memcmp($0.baseAddress!,act16.pointer.advanced(by:n*hidden*2),suffix.count)==0 }, "pair scope wrote outside normalized input")
    }
    func immutableCopies() throws {
        try require(sha(input)==inputSHA && sha(suffix)==suffixSHA, "immutable reset copies changed")
    }
}
final class Expected {
    let data: [String: Data], digests: [String: String], count: Int
    init(_ device: MTLDevice, _ state: State) throws {
        let bytes = state.outputs.reduce(0) { $0 + $1.1.bytes }
        try require(device.currentAllocatedSize + ownedCPUBytes + transientBytes + bytes <= combinedLimit, "immutable output copies exceed bound")
        var values = [String: Data]()
        for (name, b, half) in state.outputs { try b.finite(half); try b.guards(); values[name] = Data(bytes: b.pointer, count: b.bytes) }
        data = values; digests = values.mapValues { sha($0) }; count = bytes; ownedCPUBytes += bytes; try memory(device)
    }
    deinit { ownedCPUBytes -= count }
    func compare(_ state: State, full: Bool) throws {
        try state.guards()
        for (name, buffer, _) in state.outputs where full || name == "gate" || name == "up" {
            let saved = data[name]!
            try require(saved.count == buffer.bytes && saved.withUnsafeBytes { memcmp($0.baseAddress!, buffer.pointer, saved.count) == 0 }, "output bytes differ: \(name)")
        }
        if !full { try state.originalInput() }
    }
    func unchanged() throws { try require(data.mapValues { sha($0) } == digests, "immutable expectation changed") }
}
final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    var pipes = [String: MTLComputePipelineState]()
    var limitedPipes = [String: MTLComputePipelineState]()
    init() throws {
        guard let d = MTLCreateSystemDefaultDevice(), let q = d.makeCommandQueue() else { throw Failure("Metal unavailable") }
        device = d; queue = q
        let library = try d.makeLibrary(URL: root.appendingPathComponent("kernels.metallib"))
        var metadata = [[String:Any]]()
        for (name, threads) in [("clef_rms_norm_to_f16",256),("clef_gemm_64x128",128),("clef_gemm_64x128_accumulate",128),("clef_silu_mul_to_f16",256)] {
            guard let f = library.makeFunction(name: name) else { throw Failure("kernel missing: \(name)") }
            let p = try d.makeComputePipelineState(function: f)
            try require(p.threadExecutionWidth == 32 && p.maxTotalThreadsPerThreadgroup >= threads && p.staticThreadgroupMemoryLength <= d.maxThreadgroupMemoryLength, "pipeline admission failed")
            pipes[name] = p
            metadata.append(["name":name,"variant":"default","threads":threads,"requested_max_total_threads":0,"thread_execution_width":p.threadExecutionWidth,"max_total_threads":p.maxTotalThreadsPerThreadgroup,"static_threadgroup_bytes":p.staticThreadgroupMemoryLength])
            if threads == 128 {
                let descriptor = MTLComputePipelineDescriptor()
                descriptor.computeFunction = f
                try require(descriptor.maxTotalThreadsPerThreadgroup == 0 && !descriptor.threadGroupSizeIsMultipleOfThreadExecutionWidth, "unexpected descriptor defaults")
                descriptor.maxTotalThreadsPerThreadgroup = 128
                let limited = try d.makeComputePipelineState(descriptor: descriptor, options: [], reflection: nil)
                try require(limited.threadExecutionWidth == 32 && limited.maxTotalThreadsPerThreadgroup == 128 && limited.staticThreadgroupMemoryLength <= d.maxThreadgroupMemoryLength, "limited pipeline admission failed")
                limitedPipes[name] = limited
                metadata.append(["name":name,"variant":"max128","threads":threads,"requested_max_total_threads":descriptor.maxTotalThreadsPerThreadgroup,"thread_group_multiple_hint":descriptor.threadGroupSizeIsMultipleOfThreadExecutionWidth,"thread_execution_width":limited.threadExecutionWidth,"max_total_threads":limited.maxTotalThreadsPerThreadgroup,"static_threadgroup_bytes":limited.staticThreadgroupMemoryLength])
            }
        }
        try emit(["kind":"device","name":d.name,"explicit_limit_bytes":explicitLimit,"combined_limit_bytes":combinedLimit,"current_allocated_bytes":d.currentAllocatedSize,"max_threadgroup_bytes":d.maxThreadgroupMemoryLength,"pipelines":metadata])
    }
    func bind(_ e: MTLComputeCommandEncoder, _ buffers: [Buffer]) {
        for (i,b) in buffers.enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
    }
    func gemm(_ e: MTLComputeCommandEncoder, input: Buffer, weight: Buffer, output: Buffer, n: Int, m: Int, k: Int, accumulate: Bool, limited: Bool) {
        e.memoryBarrier(scope: .buffers)
        e.setComputePipelineState((limited ? limitedPipes : pipes)[accumulate ? "clef_gemm_64x128_accumulate" : "clef_gemm_64x128"]!)
        bind(e,[input,weight,output]); var dims = SIMD4<UInt32>(UInt32(n),UInt32(m),UInt32(k),0)
        e.setBytes(&dims,length:16,index:3)
        e.dispatchThreadgroups(MTLSize(width:(m+127)/128,height:(n+63)/64,depth:1),threadsPerThreadgroup:MTLSize(width:128,height:1,depth:1)); dispatches += 1
    }
    func run(_ state: State, _ weights: Weights, limited: Bool, full: Bool) throws -> (Double,Double) {
        try deadline()
        return try autoreleasepool {
            let began = ProcessInfo.processInfo.systemUptime
            guard let cb = queue.makeCommandBuffer(), let e = cb.makeComputeCommandEncoder(dispatchType: .concurrent) else { throw Failure("command allocation failed") }
            e.setComputePipelineState(pipes["clef_rms_norm_to_f16"]!); bind(e,[state.x,weights.norm,state.act16])
            var d = UInt32(hidden), eps: Float = 1e-6
            e.setBytes(&d,length:4,index:3);e.setBytes(&eps,length:4,index:4)
            e.dispatchThreadgroups(MTLSize(width:state.n,height:1,depth:1),threadsPerThreadgroup:MTLSize(width:256,height:1,depth:1));dispatches += 1
            gemm(e,input:state.act16,weight:weights.gate,output:state.gate,n:state.n,m:wide,k:hidden,accumulate:false,limited:limited)
            // Every dependency barrier is identical; only pipeline compilation differs.
            gemm(e,input:state.act16,weight:weights.up,output:state.up,n:state.n,m:wide,k:hidden,accumulate:false,limited:limited)
            if full {
                // This barrier also protects act16 from being overwritten while either GEMM reads it.
                e.memoryBarrier(scope:.buffers);e.setComputePipelineState(pipes["clef_silu_mul_to_f16"]!)
                bind(e,[state.gate,state.up,state.act16]);var count=UInt64(state.n*wide);e.setBytes(&count,length:8,index:3)
                e.dispatchThreadgroups(MTLSize(width:(state.n*wide+255)/256,height:1,depth:1),threadsPerThreadgroup:MTLSize(width:256,height:1,depth:1));dispatches += 1
                gemm(e,input:state.act16,weight:weights.down,output:state.x,n:state.n,m:hidden,k:wide,accumulate:true,limited:limited)
            }
            e.endEncoding();cb.commit();commands += 1;cb.waitUntilCompleted()
            try require(cb.status == .completed,"command failed: \(String(describing:cb.error))")
            let elapsed=ProcessInfo.processInfo.systemUptime-began,gpu=cb.gpuEndTime-cb.gpuStartTime
            try require(gpu.isFinite && gpu>0 && elapsed.isFinite && elapsed>0,"invalid timestamps")
            try memory(device);try deadline();return(gpu,elapsed)
        }
    }
}

do {
    let r=try Runner(),w=try Weights(r.device)
    for n in [1,17,63,64,65,512,1024,2048] {
        try autoreleasepool {
            let s=try State(r.device,n);s.reset()
            _=try r.run(s,w,limited:false,full:true);let expected=try Expected(r.device,s)
            try emit(["kind":"correctness","n":n,"variant":"default","outputs_sha256":expected.digests,"all_outputs_finite":true])
            s.reset();_=try r.run(s,w,limited:true,full:true);try expected.compare(s,full:true)
            try emit(["kind":"correctness","n":n,"variant":"max128","all_outputs_bitwise_equal":true,"output_bytes":expected.count])
            if n>=512 {
                for round in 0..<5 {
                    for scope in ["normalized_pair","full_ffn"] {
                        for (position,limited) in [false,true,true,false].enumerated() {
                            s.reset();let t=try r.run(s,w,limited:limited,full:scope=="full_ffn")
                            try expected.compare(s,full:scope=="full_ffn")
                            timingRows += 1
                            try emit(["kind":"timing","n":n,"round":round,"scope":scope,"position":position,"variant":limited ? "max128":"default","gpu_seconds":t.0,"host_seconds":t.1,"exact_output_check_passed":true])
                        }
                    }
                }
            }
            try expected.unchanged();try s.immutableCopies();try w.unchanged();try s.guards()
            try emit(["kind":"case_complete","n":n,"immutable_weights_and_expectations":true,"redzones":true,"explicit_live_bytes":liveBytes,"owned_cpu_bytes":ownedCPUBytes,"device_current_allocated_bytes":r.device.currentAllocatedSize])
        }
    }
    try w.unchanged();try require(commands==136 && dispatches==560 && timingRows==120,"fixed coverage differs")
    try require(ownedCPUBytes==0,"owned CPU copy lifetime did not end")
    try emit(["kind":"complete","commands":commands,"dispatches":dispatches,"timing_rows":timingRows,"explicit_peak_bytes":peakBytes,"combined_peak_bytes":peakCombined,"owned_cpu_bytes_after_cases":ownedCPUBytes,"q4_conversion_executed":false,"numerical_gate":"all output bits equal","speed_qualification":"full_ffn paired ratio <=0.90 at every timed size; evaluated offline" ])
} catch {
    try? emit(["kind":"failure","message":String(describing:error),"commands":commands,"dispatches":dispatches,"timing_rows":timingRows,"explicit_peak_bytes":peakBytes,"combined_peak_bytes":peakCombined])
    exit(1)
}
