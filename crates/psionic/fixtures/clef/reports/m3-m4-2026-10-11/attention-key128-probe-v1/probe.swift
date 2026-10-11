// One attention key-tile candidate. No model weights or host settings are used.
import Foundation
import Metal
import CryptoKit
import Darwin

struct Failure: Error, CustomStringConvertible { let description: String; init(_ s: String) { description = s } }
func require(_ b: Bool, _ s: String) throws { if !b { throw Failure(s) } }
func emit(_ value: [String: Any]) throws {
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]))
    FileHandle.standardOutput.write(Data([10]))
}
func hash(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
func loadAverage() -> [Double] { var a = [Double](repeating: 0, count: 3); return a.withUnsafeMutableBufferPointer { getloadavg($0.baseAddress!, 3) } == 3 ? a : [] }
let args = CommandLine.arguments
try require(args.count == 3 && ["check", "bench"].contains(args[2]), "Usage: probe DIRECTORY check|bench")
let root = URL(fileURLWithPath: args[1]), benchmark = args[2] == "bench"
let started = ProcessInfo.processInfo.systemUptime
let explicitLimit = 1024 * 1024 * 1024, driverLimit = 1536 * 1024 * 1024, pad = 256, dim = 256
var liveBytes = 0, peakBytes = 0, commands = 0, dispatches = 0, comparisons = 0, checks = 0
func timeCheck() throws { try require(ProcessInfo.processInfo.systemUptime - started < 50, "internal 50-second deadline reached") }
final class Buffer {
    let metal: MTLBuffer, bytes: Int, tail: Int
    var pointer: UnsafeMutableRawPointer { metal.contents().advanced(by: pad) }
    init(_ device: MTLDevice, _ bytes: Int, tail: Int = pad) throws {
        let total = pad + bytes + tail
        try require(bytes > 0 && total <= explicitLimit && liveBytes <= explicitLimit - total, "explicit buffer limit exceeded")
        guard let metal = device.makeBuffer(length: total, options: [.storageModeShared, .hazardTrackingModeTracked]) else { throw Failure("allocation failed") }
        self.metal = metal; self.bytes = bytes; self.tail = tail
        liveBytes += total; peakBytes = max(peakBytes, liveBytes); memset(metal.contents(), 0xa5, total)
        try require(device.currentAllocatedSize <= driverLimit, "driver allocation limit exceeded")
    }
    deinit { liveBytes -= pad + bytes + tail }
    func digest() -> String { hash(Data(bytesNoCopy: metal.contents(), count: pad + bytes + tail, deallocator: .none)) }
    func guards() throws {
        let p = metal.contents().assumingMemoryBound(to: UInt8.self)
        for i in 0..<pad { try require(p[i] == 0xa5, "prefix redzone changed") }
        for i in 0..<tail { try require(p[pad + bytes + i] == 0xa5, "suffix redzone changed") }
    }
    func poison() { pointer.assumingMemoryBound(to: Float.self).initialize(repeating: Float(bitPattern: 0x7fc00555), count: bytes / 4) }
}
struct Shape { let n, heads, kv, first: Int; var keys: Int { first + n }; var count: Int { n * heads * dim } }
final class Inputs {
    let shape: Shape, q: Buffer, k: Buffer, v: Buffer
    let digests: [String]
    init(_ device: MTLDevice, _ s: Shape, pattern: String = "random") throws {
        shape = s
        q = try Buffer(device, s.n * s.heads * dim * 2)
        k = try Buffer(device, s.keys * s.kv * dim * 2)
        v = try Buffer(device, s.keys * s.kv * dim * 2)
        var state: UInt64 = 0x25caa78626
        for (index, b) in [q, k, v].enumerated() {
            let p = b.pointer.assumingMemoryBound(to: Float16.self)
            for i in 0..<(b.bytes / 2) {
                state ^= state << 13; state ^= state >> 7; state ^= state << 17
                let scale: Float = index == 0 ? 0.125 : (index == 1 ? 0.5 : 1)
                p[i] = Float16(Float(Int(state & 65535) - 32768) / 32768 * scale)
            }
        }
        let qp = q.pointer.assumingMemoryBound(to: Float16.self), kp = k.pointer.assumingMemoryBound(to: Float16.self), vp = v.pointer.assumingMemoryBound(to: Float16.self)
        if pattern == "uniform" {
            qp.initialize(repeating: 0, count: q.bytes / 2); kp.initialize(repeating: 0, count: k.bytes / 2)
            vp.initialize(repeating: 1, count: v.bytes / 2)
        } else if ["rising", "falling", "extreme"].contains(pattern) {
            qp.initialize(repeating: 0, count: q.bytes / 2); kp.initialize(repeating: 0, count: k.bytes / 2)
            for row in 0..<s.n { for h in 0..<s.heads { qp[(row * s.heads + h) * dim] = 1 } }
            for key in 0..<s.keys { for h in 0..<s.kv {
                let tile = min(2, key / 128), base: [Float] = pattern == "rising" ? [96,104,112] : (pattern == "falling" ? [112,104,96] : [-96,0,96])
                let offset = (key * s.kv + h) * dim
                kp[offset] = Float16(base[tile] + Float(key % 32) / 8 + Float(h) / 4)
                for d in 0..<dim { vp[offset + d] = Float16([-0.75,0.25,0.875][tile] + Float(key % 5) / 16 + Float(d % 7) / 128 + Float(h) / 8) }
            }}
        } else if pattern == "future" {
            let first = (s.first + 1) * s.kv * dim
            for i in first..<(k.bytes / 2) { kp[i] = 16; vp[i] = 16 }
        }
        digests = [q,k,v].map { $0.digest() }
    }
    func unchanged() throws { for (b,h) in zip([q,k,v],digests) { try b.guards(); try require(b.digest() == h, "attention input changed") } }
}
struct Timing { let gpu, wall: Double }
final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    var pipelines = [Int: MTLComputePipelineState]()
    init() throws {
        guard let device = MTLCreateSystemDefaultDevice(), let queue = device.makeCommandQueue() else { throw Failure("Metal unavailable") }
        self.device = device; self.queue = queue
        let library = try device.makeLibrary(URL: root.appendingPathComponent("kernels.metallib"))
        for keys in [32,128] {
            let name = keys == 32 ? "clef_flash_attention256_tensorops" : "clef_flash_attention256_key128"
            guard let f = library.makeFunction(name: name) else { throw Failure("kernel unavailable") }
            let p = try device.makeComputePipelineState(function: f)
            let declared = 16 * keys * 6 + 16 * dim * 4
            try require(p.threadExecutionWidth == 32 && p.maxTotalThreadsPerThreadgroup >= 128, "four32-thread SIMD groups unavailable")
            try require(declared <= device.maxThreadgroupMemoryLength && p.staticThreadgroupMemoryLength <= device.maxThreadgroupMemoryLength, "threadgroup memory exceeds device cap")
            pipelines[keys] = p
            try emit(["kind":"pipeline", "keys":keys, "query_tile":16, "threads":128, "declared_threadgroup_bytes":declared, "compiled_threadgroup_bytes":p.staticThreadgroupMemoryLength, "device_max_threadgroup_bytes":device.maxThreadgroupMemoryLength, "max_threads":p.maxTotalThreadsPerThreadgroup, "thread_execution_width":p.threadExecutionWidth])
        }
        try emit(["kind":"device", "name":device.name, "load_average":loadAverage(), "recommended_working_set_bytes":device.recommendedMaxWorkingSetSize, "explicit_limit_bytes":explicitLimit, "driver_limit_bytes":driverLimit])
    }
    func run(_ keys: Int, _ input: Inputs, _ output: Buffer, offset: Int = 0, count: Int? = nil) throws -> Timing {
        try timeCheck()
        let s = input.shape, n = count ?? s.n, bytes = n * s.heads * dim * 4
        try require(offset >= 0 && n > 0 && offset + n <= s.n && output.bytes >= bytes, "invalid dispatch slice")
        output.poison()
        return try autoreleasepool {
            let start = ProcessInfo.processInfo.systemUptime
            guard let cb = queue.makeCommandBuffer(), let e = cb.makeComputeCommandEncoder() else { throw Failure("command allocation failed") }
            e.setComputePipelineState(pipelines[keys]!)
            e.setBuffer(input.q.metal, offset: pad + offset * s.heads * dim * 2, index: 0)
            e.setBuffer(input.k.metal, offset: pad, index: 1); e.setBuffer(input.v.metal, offset: pad, index: 2)
            e.setBuffer(output.metal, offset: pad, index: 3)
            var a = SIMD4<UInt32>(UInt32(n), UInt32(s.heads), UInt32(s.kv), UInt32(s.first + offset))
            e.setBytes(&a, length: 16, index: 4)
            e.dispatchThreadgroups(MTLSize(width:(n+15)/16,height:s.heads,depth:1), threadsPerThreadgroup:MTLSize(width:128,height:1,depth:1))
            e.endEncoding(); cb.commit(); cb.waitUntilCompleted(); commands += 1; dispatches += 1
            try require(cb.status == .completed, "GPU command failed: \(String(describing:cb.error))")
            let gpu = cb.gpuEndTime - cb.gpuStartTime
            try require(gpu.isFinite && gpu > 0, "invalid GPU timestamp")
            try require(device.currentAllocatedSize <= driverLimit, "driver allocation limit exceeded")
            return Timing(gpu:gpu, wall:ProcessInfo.processInfo.systemUptime-start)
        }
    }
}
@discardableResult func compare(_ actual: Buffer, _ expected: UnsafePointer<Float>, count: Int, label: String, absolute: Double = 0.001, relative: Double = 0) throws -> Double {
    try actual.guards()
    let a = actual.pointer.assumingMemoryBound(to: Float.self)
    var maximum = 0.0, total = 0.0, bad = 0, first = -1, firstActual = 0.0, firstExpected = 0.0, bits = 0
    for i in 0..<count {
        let x = Double(a[i]), y = Double(expected[i]), error = abs(x-y)
        if a[i].bitPattern != expected[i].bitPattern { bits += 1 }
        if !x.isFinite || !y.isFinite || error > absolute + relative * abs(y) {
            bad += 1
            if first < 0 { first=i; firstActual=x.isFinite ? x : 0; firstExpected=y.isFinite ? y : 0 }
        }
        if error.isFinite { maximum=max(maximum,error); total+=error }
    }
    comparisons += count; checks += 1
    try emit(["kind":"comparison", "label":label, "values":count, "max_abs":maximum, "mean_abs":total/Double(count), "absolute_limit":absolute, "relative_limit":relative, "different_bits":bits, "failures":bad, "first_failure_index":first, "first_actual_finite_or_zero":firstActual, "first_expected_finite_or_zero":firstExpected, "passed":bad==0])
    try require(bad == 0, "attention numerical gate failed: \(label)")
    return maximum
}
// Both immutable expectations were already checked for finite values in the full
// cross-variant scan. Equal bytes therefore prove every repeated value is finite.
func exactRepeat(_ actual: Buffer, _ expected: Buffer, label: String) throws {
    try require(actual.bytes == expected.bytes, "repeat buffer lengths differ")
    try actual.guards(); try expected.guards()
    if memcmp(actual.pointer, expected.pointer, actual.bytes) != 0 {
        try compare(actual, expected.pointer.assumingMemoryBound(to: Float.self), count: actual.bytes / 4, label: label, absolute: 0)
        throw Failure("same-variant output differs in bits: \(label)")
    }
    comparisons += actual.bytes / 4; checks += 1
    try emit(["kind":"comparison", "label":label, "values":actual.bytes / 4, "max_abs":0, "different_bits":0, "finite_by_exact_prechecked_bytes":true, "failures":0, "passed":true])
}
// Independent dense f64 reference using only the stored half input values.
func oracle(_ input: Inputs, row: Int, head: Int) -> [Float] {
    let s=input.shape, qp=input.q.pointer.assumingMemoryBound(to:Float16.self), kp=input.k.pointer.assumingMemoryBound(to:Float16.self), vp=input.v.pointer.assumingMemoryBound(to:Float16.self)
    let qbase=(row*s.heads+head)*dim, kh=head/(s.heads/s.kv), visible=s.first+row+1
    var scores=[Double](repeating:0,count:visible)
    for key in 0..<visible { var sum=0.0; let base=(key*s.kv+kh)*dim; for d in 0..<dim { sum += Double(qp[qbase+d])*Double(kp[base+d]) }; scores[key]=sum }
    let maximum=scores.max()!; var denominator=0.0
    for key in 0..<visible { scores[key]=exp(scores[key]-maximum); denominator+=scores[key] }
    var result=[Float](repeating:0,count:dim)
    for d in 0..<dim { var sum=0.0; for key in 0..<visible { sum += scores[key]*Double(vp[(key*s.kv+kh)*dim+d]) }; result[d]=Float(sum/denominator) }
    return result
}
func oracleAll(_ input: Inputs) throws -> [Float] {
    var result=[Float](); result.reserveCapacity(input.shape.count)
    for row in 0..<input.shape.n { try timeCheck(); for head in 0..<input.shape.heads { result.append(contentsOf:oracle(input,row:row,head:head)) } }
    return result
}
func output(_ device: MTLDevice, _ s: Shape, count: Int? = nil) throws -> Buffer { try Buffer(device,(count ?? s.n)*s.heads*dim*4,tail:16*s.heads*dim*4) }
let runner = try Runner()
var cases:[(Shape,String)]=[]
for (n,first) in [(1,0),(15,31),(16,32),(17,127),(33,128),(17,129),(1,255)] { cases.append((Shape(n:n,heads:6,kv:2,first:first),"random")) }
for pattern in ["uniform","rising","falling","extreme","future"] { cases.append((Shape(n:17,heads:6,kv:2,first:256),pattern)) }
for (index,item) in cases.enumerated() {
    try autoreleasepool {
        let (s,pattern)=item, input=try Inputs(runner.device,s,pattern:pattern)
        let expected=try oracleAll(input), base=try output(runner.device,s), candidate=try output(runner.device,s)
        _=try runner.run(32,input,base); _=try runner.run(128,input,candidate)
        try expected.withUnsafeBufferPointer { p in
            try compare(base,p.baseAddress!,count:s.count,label:"small\(index)-32-f64")
            try compare(candidate,p.baseAddress!,count:s.count,label:"small\(index)-128-f64")
        }
        try compare(candidate,base.pointer.assumingMemoryBound(to:Float.self),count:s.count,label:"small\(index)-128-32")
        try input.unchanged()
        try emit(["kind":"small_complete","index":index,"n":s.n,"first":s.first,"heads":s.heads,"kv_heads":s.kv,"pattern":pattern])
    }
}
// Each implementation must repeat exactly and preserve tight chunk equivalence.
try autoreleasepool {
    let s=Shape(n:65,heads:6,kv:2,first:127), input=try Inputs(runner.device,s)
    for keys in [32,128] {
        let whole=try output(runner.device,s), repeatOut=try output(runner.device,s)
        _=try runner.run(keys,input,whole); _=try runner.run(keys,input,repeatOut)
        try compare(repeatOut,whole.pointer.assumingMemoryBound(to:Float.self),count:s.count,label:"repeat-\(keys)",absolute:0)
        try require(memcmp(repeatOut.pointer,whole.pointer,whole.bytes)==0,"repeat must be bitwise exact")
        for chunk in [15,17,31,33] { for start in stride(from:0,to:s.n,by:chunk) {
            let n=min(chunk,s.n-start), part=try output(runner.device,s,count:n)
            _=try runner.run(keys,input,part,offset:start,count:n)
            try compare(part,whole.pointer.assumingMemoryBound(to:Float.self).advanced(by:start*s.heads*dim),count:n*s.heads*dim,label:"chunk-\(keys)-\(chunk)-\(start)",absolute:2e-5,relative:5e-5)
        }}
    }
    try input.unchanged()
}
// Final 2,048-token chunk at representative history lengths. This is not a whole-model timing.
for heads in [16,24] { for total in [1024,4096,16384] {
    try autoreleasepool {
        let n=min(2048,total), s=Shape(n:n,heads:heads,kv:4,first:total-n), input=try Inputs(runner.device,s)
        let base=try output(runner.device,s), candidate=try output(runner.device,s), reference=try output(runner.device,s)
        _=try runner.run(32,input,reference); _=try runner.run(128,input,candidate)
        try compare(candidate,reference.pointer.assumingMemoryBound(to:Float.self),count:s.count,label:"large-\(heads)-\(total)-128-32")
        // Fixed rows/heads: full256-channel f64 oracles, chosen before GPU output exists.
        for row in [0,n/2,n-1] { for head in [0,heads-1] {
            let expected=oracle(input,row:row,head:head), offset=(row*heads+head)*dim
            for (keys,b) in [(32,reference),(128,candidate)] {
                let a=b.pointer.assumingMemoryBound(to:Float.self).advanced(by:offset)
                var maximum=0.0,bad=0
                for d in 0..<dim { let error=abs(Double(a[d])-Double(expected[d])); if error.isFinite { maximum=max(maximum,error) }; if !a[d].isFinite || !error.isFinite || error>0.001 { bad+=1 } }
                comparisons+=dim;checks+=1
                try emit(["kind":"sample_oracle","heads":heads,"total":total,"row":row,"head":head,"keys":keys,"values":dim,"max_abs":maximum,"failures":bad,"passed":bad==0])
                try require(bad==0,"large fixed f64 oracle failed")
            }
        }}
        let referenceHash=reference.digest(), candidateHash=candidate.digest()
        if benchmark {
            for round in 0..<5 { for keys in (round % 2 == 0 ? [32,128] : [128,32]) {
                let b=base, timing=try runner.run(keys,input,base)
                // Full checks occur after the command, outside its timestamp and wall interval.
                try exactRepeat(b, keys==32 ? reference : candidate, label:"timed-\(heads)-\(total)-\(round)-\(keys)")
                try emit(["kind":"timing","heads":heads,"kv_heads":4,"n":n,"first":s.first,"total":total,"round":round,"keys":keys,"gpu_seconds":timing.gpu,"wall_seconds":timing.wall,"device_allocated_bytes":runner.device.currentAllocatedSize,"explicit_live_bytes":liveBytes,"load_average":loadAverage()])
            }}
        }
        try input.unchanged(); try reference.guards(); try candidate.guards(); try require(reference.digest()==referenceHash && candidate.digest()==candidateHash,"immutable expected output mutated")
        try emit(["kind":"large_complete","heads":heads,"total":total,"n":n,"first":s.first,"explicit_live_bytes":liveBytes,"device_allocated_bytes":runner.device.currentAllocatedSize])
    }
}}
try require(commands == (benchmark ? 128 : 68) && dispatches == commands, "fixed command count differs")
try emit(["kind":"complete","mode":args[2],"commands":commands,"dispatches":dispatches,"checks":checks,"compared_values":comparisons,"peak_explicit_bytes":peakBytes,"device_allocated_bytes":runner.device.currentAllocatedSize,"seconds":ProcessInfo.processInfo.systemUptime-started,"passed":true])
