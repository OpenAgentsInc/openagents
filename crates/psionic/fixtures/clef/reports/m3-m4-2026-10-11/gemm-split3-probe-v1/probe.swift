// One three-part parallel K reduction; weights retain the original layout.
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
func kernel(_ candidate: Bool, _ m: Int, _ k: Int, _ accumulate: Bool) -> String {
    let t = tile(m, k)
    let stem = t == 64 ? "clef_gemm" : "clef_gemm_64x\(t)"
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
        for t in [32, 64, 128] { for acc in [false, true] {
            let stem = t == 64 ? "clef_gemm" : "clef_gemm_64x\(t)"
            let name = stem + (acc ? "_accumulate" : "")
            guard let function = library.makeFunction(name: name) else { throw Failure("kernel missing: \(name)") }
            let pipeline = try device.makeComputePipelineState(function: function)
            try require(pipeline.threadExecutionWidth == 32 && pipeline.maxTotalThreadsPerThreadgroup >= 128, "pipeline threads unsupported")
            try require(pipeline.staticThreadgroupMemoryLength <= device.maxThreadgroupMemoryLength, "pipeline shared memory unsupported")
            pipelines[name] = pipeline
        }}
        for name in ["clef_split3_partial", "clef_split3_reduce"] {
            guard let function = library.makeFunction(name: name) else { throw Failure("split kernel missing") }
            let pipeline = try device.makeComputePipelineState(function: function)
            try require(pipeline.threadExecutionWidth == 32 && pipeline.maxTotalThreadsPerThreadgroup >= 128, "split threads unsupported")
            try require(pipeline.staticThreadgroupMemoryLength <= device.maxThreadgroupMemoryLength, "split shared memory unsupported")
            pipelines[name] = pipeline
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
    func gemm(_ e: MTLComputeCommandEncoder, _ candidate: Bool, x: Buffer, w: Buffer, y: Buffer, n: Int, m: Int, k: Int, acc: Bool = false, partials: Partials? = nil) throws {
        try require(n > 0 && m > 0 && k > 0 && x.bytes == n*k*2 && w.bytes == m*k*2 && y.bytes == n*m*4, "matrix binding length differs")
        e.memoryBarrier(scope: .buffers)
        if candidate && k == 12288 {
            guard let partials = partials else { throw Failure("parallel down requires owned partial buffers") }
            try require(partials.n == n && partials.m == m, "partial shape differs")
            e.setComputePipelineState(pipelines["clef_split3_partial"]!)
            for (i, b) in ([x, w] + partials.buffers).enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
            var args = SIMD4<UInt32>(UInt32(n), UInt32(m), UInt32(k), 0)
            e.setBytes(&args, length: 16, index: 5)
            let groups = 3 * ((m + 127) / 128) * ((n + 63) / 64)
            e.dispatchThreadgroups(MTLSize(width: groups, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1)); dispatches += 1
            e.memoryBarrier(scope: .buffers)
            e.setComputePipelineState(pipelines["clef_split3_reduce"]!)
            for (i, b) in (partials.buffers + [y]).enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
            var reduction = SIMD2<UInt32>(UInt32(n*m), acc ? 1 : 0)
            e.setBytes(&reduction, length: 8, index: 4)
            e.dispatchThreadgroups(MTLSize(width: (n*m+127)/128, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1)); dispatches += 1
        } else {
            e.setComputePipelineState(pipelines[kernel(false, m, k, acc)]!)
            for (i, b) in [x, w, y].enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
            var args = SIMD4<UInt32>(UInt32(n), UInt32(m), UInt32(k), 0)
            e.setBytes(&args, length: 16, index: 3)
            e.dispatchThreadgroups(MTLSize(width: (m+tile(m,k)-1)/tile(m,k), height: (n+63)/64, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1)); dispatches += 1
        }
    }
    func submit(_ f: (MTLComputeCommandEncoder) throws -> Void) throws -> Timing {
        try timeCheck()
        return try autoreleasepool {
            let began = ProcessInfo.processInfo.systemUptime
            guard let cb = queue.makeCommandBuffer(), let e = cb.makeComputeCommandEncoder(dispatchType: .concurrent) else { throw Failure("command allocation failed") }
            try f(e); e.endEncoding(); cb.commit(); cb.waitUntilCompleted(); commands += 1
            try require(cb.status == .completed, "GPU command failed: \(String(describing: cb.error))")
            try require(device.currentAllocatedSize <= driverLimit, "private Metal allocation exceeded limit")
            let gpu = cb.gpuEndTime - cb.gpuStartTime
            try require(gpu.isFinite && gpu > 0, "invalid GPU timestamp")
            return Timing(gpu: gpu, wall: ProcessInfo.processInfo.systemUptime - began)
        }
    }
    func ffn(_ candidate: Bool, _ weights: Weights, _ x: Buffer, _ state: State, n: Int, acc: Bool = true) throws -> Timing {
        try submit { e in
            try gemm(e, candidate, x: x, w: weights.gate, y: state.gate, n: n, m: 12288, k: 4096)
            try gemm(e, candidate, x: x, w: weights.up, y: state.up, n: n, m: 12288, k: 4096)
            e.memoryBarrier(scope: .buffers); e.setComputePipelineState(pipelines["silu"]!)
            for (i, b) in [state.gate, state.up, state.activation].enumerated() { e.setBuffer(b.metal, offset: pad, index: i) }
            var count = UInt64(n * 12288); e.setBytes(&count, length: 8, index: 3)
            e.dispatchThreadgroups(MTLSize(width: (Int(count) + 127) / 128, height: 1, depth: 1), threadsPerThreadgroup: MTLSize(width: 128, height: 1, depth: 1)); dispatches += 1
            try gemm(e, candidate, x: state.activation, w: weights.down, y: state.output, n: n, m: 4096, k: 12288, acc: acc, partials: state.partials)
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
final class Partials {
    let n, m: Int
    let buffers: [Buffer]
    static func validate(n: Int, m: Int, k: Int) throws {
        try require(n > 0 && n <= 2048 && m > 0 && m <= 12288 && k == 12288, "split3 requires bounded positive rows/columns and K12288")
    }
    init(_ device: MTLDevice, n: Int, m: Int, k: Int = 12288) throws {
        try Partials.validate(n: n, m: m, k: k)
        self.n = n; self.m = m
        buffers = try (0..<3).map { _ in try Buffer(device, n*m*4) }
    }
    func reset() { for b in buffers { b.poisonFloat() } }
    func guards() throws { for b in buffers { try b.guards() } }
}
final class State {
    let gate, up, activation, output: Buffer
    let partials: Partials?
    var buffers: [Buffer] { [gate, up, activation, output] }
    init(_ device: MTLDevice, _ n: Int, split: Bool) throws {
        gate = try Buffer(device, n*12288*4); up = try Buffer(device, n*12288*4)
        activation = try Buffer(device, n*12288*2); output = try Buffer(device, n*4096*4)
        partials = split ? try Partials(device, n: n, m: 4096) : nil
    }
    func reset(_ acc: Bool = true) {
        gate.poisonFloat(); up.poisonFloat(); activation.poisonHalf(); partials?.reset()
        if acc { residual(output) } else { output.poisonFloat() }
    }
    func guards() throws { for b in buffers { try b.guards() }; try partials?.guards() }
}
func absolute(_ a: Buffer, _ b: Buffer) throws -> [String:Any] {
    try require(a.bytes == b.bytes, "output comparison length differs")
    try a.guards(); try b.guards()
    let p=a.pointer.assumingMemoryBound(to:Float.self),q=b.pointer.assumingMemoryBound(to:Float.self)
    var maximum=0.0, differences=0, over=0
    var first=[[String:Any]]()
    for i in 0..<(a.bytes/4) {
        if !(p[i].isFinite && q[i].isFinite) {
            try emit(["kind":"nonfinite_failure","index":i,"reference_bits":p[i].bitPattern,"candidate_bits":q[i].bitPattern])
            throw Failure("nonfinite/unwritten output")
        }
        let error=abs(Double(p[i])-Double(q[i]));maximum=max(maximum,error)
        if p[i].bitPattern != q[i].bitPattern { differences += 1 }
        if error > 0.001 { over += 1; if first.count<8 {first.append(["index":i,"reference":p[i],"candidate":q[i],"error":error])} }
    }
    let result:[String:Any]=["values":a.bytes/4,"max_abs":maximum,"bit_differences":differences,"above_0_001":over,"first_failures":first,"passed":over==0]
    if over>0 {try emit(["kind":"absolute_failure","comparison":result]);throw Failure("full output exceeded fixed absolute0.001 bound")}
    return result
}
func residualValue(_ i:Int)->Float { Float((i*13+17)%101-50)/37 }
func oracle(_ x:Buffer,_ w:Buffer,_ baseline:Buffer,_ candidate:Buffer,_ partials:Partials,n:Int,m:Int,acc:Bool,all:Bool) throws -> [String:Any] {
    let xp=x.pointer.assumingMemoryBound(to:Float16.self),wp=w.pointer.assumingMemoryBound(to:Float16.self)
    let a=baseline.pointer.assumingMemoryBound(to:Float.self),b=candidate.pointer.assumingMemoryBound(to:Float.self)
    let pp=partials.buffers.map{$0.pointer.assumingMemoryBound(to:Float.self)}
    let indices=all ? Array(0..<(n*m)) : (0..<96).map { s in s<4 ? ((s%2 == 0 ? 0:n-1)*m+(s<2 ? 0:m-1)) : ((s*97+5)%n)*m+(s*997+11)%m }
    var maxBaseline=0.0,maxCandidate=0.0,maxPartial=[Double](repeating:0,count:3)
    for index in indices {
        let row=index/m,col=index%m
        var total=0.0, expected=[Double](repeating:0,count:3)
        for k in 0..<12288 {
            let product=Double(xp[row*12288+k])*Double(wp[col*12288+k])
            total += product;expected[k/4096] += product
        }
        if acc {total += Double(residualValue(index))}
        let ea=abs(Double(a[index])-total),eb=abs(Double(b[index])-total)
        maxBaseline=max(maxBaseline,ea);maxCandidate=max(maxCandidate,eb)
        if !(a[index].isFinite && b[index].isFinite && ea<=0.001 && eb<=0.001) {
            try emit(["kind":"oracle_failure","scope":"whole_dot","row":row,"column":col,"expected":total,"baseline":a[index],"candidate":b[index],"baseline_error":ea,"candidate_error":eb])
            throw Failure("f64 whole-dot oracle exceeded fixed absolute0.001 bound")
        }
        for part in 0..<3 {
            let ep=abs(Double(pp[part][index])-expected[part]);maxPartial[part]=max(maxPartial[part],ep)
            if !(pp[part][index].isFinite && ep<=0.001) {
                try emit(["kind":"oracle_failure","scope":"partial","partition":part,"row":row,"column":col,"expected":expected[part],"actual_bits":pp[part][index].bitPattern,"error_text":String(ep)])
                throw Failure("independent partial oracle exceeded fixed absolute0.001 bound")
            }
        }
    }
    try partials.guards()
    return ["sample_count":indices.count,"all_outputs":all,"baseline_max_abs":maxBaseline,"candidate_max_abs":maxCandidate,"partial_max_abs":maxPartial,"passed":true]
}
func fixture(_ x:Buffer,_ w:Buffer,n:Int,m:Int,kind:String) {
    fillHalf(x,width:12288,seed:19,scale:0.125);fillHalf(w,width:12288,seed:41,scale:1/sqrt(Float(12288)))
    let xp=x.pointer.assumingMemoryBound(to:Float16.self),wp=w.pointer.assumingMemoryBound(to:Float16.self)
    if kind=="zero" {xp.initialize(repeating:0,count:n*12288);wp.initialize(repeating:0,count:m*12288)}
    if kind=="correlated" {
        for i in 0..<(n*12288) {xp[i]=Float16(Float(i%17-8)/64)}
        for i in 0..<(m*12288) {wp[i]=Float16(Float(i%17-8)/128)}
    }
    if kind=="cancellation" {
        for i in 0..<(n*12288) {xp[i]=Float16(0.25)}
        for i in 0..<(m*12288) {wp[i]=Float16((i%2==0 ? 1.0:-1.0)/64)}
    }
    if kind=="mixedrange" {
        for i in 0..<(n*12288) {xp[i]=Float16(i%29==0 ? 4:Float(i%7-3)/1024)}
        for i in 0..<(m*12288) {wp[i]=Float16(i%31==0 ? 0.03125:Float(i%11-5)/16384)}
    }
    if kind=="partition_sentinels" {
        for i in 0..<(n*12288) {xp[i]=Float16(Float((i%12288)/4096+1)/8)}
        for i in 0..<(m*12288) {wp[i]=Float16([Float(0.125),-0.0625,0.03125][(i%12288)/4096])}
    }
}
let smallCases:[(Int,Int,String)]=[(1,1,"zero"),(3,7,"nonsymmetric"),(7,65,"correlated"),(63,1,"cancellation"),(64,1,"mixedrange"),(65,1,"partition_sentinels"),(257,1,"nonsymmetric")]
func small(_ runner:Runner,n:Int,m:Int,kind:String,acc:Bool)throws {
    try autoreleasepool {
        let x=try Buffer(runner.device,n*12288*2),w=try Buffer(runner.device,m*12288*2)
        let a=try Buffer(runner.device,n*m*4),b=try Buffer(runner.device,n*m*4),p=try Partials(runner.device,n:n,m:m)
        fixture(x,w,n:n,m:m,kind:kind);let hashes=[x.digest(),w.digest()]
        if acc {residual(a);residual(b)} else {a.poisonFloat();b.poisonFloat()};p.reset()
        _=try runner.submit{try runner.gemm($0,false,x:x,w:w,y:a,n:n,m:m,k:12288,acc:acc)}
        _=try runner.submit{try runner.gemm($0,true,x:x,w:w,y:b,n:n,m:m,k:12288,acc:acc,partials:p)}
        let comparison=try absolute(a,b),reference=try oracle(x,w,a,b,p,n:n,m:m,acc:acc,all:true)
        try require([x.digest(),w.digest()]==hashes,"synthetic immutable input changed")
        try emit(["kind":"small_correctness","n":n,"m":m,"k":12288,"fixture":kind,"accumulate":acc,"comparison":comparison,"f64_oracle":reference,"passed":true,"guards_and_inputs_unchanged":true])
    }
}
func weighted(_ runner:Runner,_ weights:Weights,n:Int,acc:Bool)throws {
    try autoreleasepool {
        let x=try Buffer(runner.device,n*4096*2);fillHalf(x,width:4096,seed:19,scale:1)
        let original=x.digest(),a=try State(runner.device,n,split:false),b=try State(runner.device,n,split:true)
        a.reset(acc);b.reset(acc)
        _=try runner.ffn(false,weights,x,a,n:n,acc:acc);_=try runner.ffn(true,weights,x,b,n:n,acc:acc)
        let gate=try exact(a.gate,b.gate),up=try exact(a.up,b.up),activation=try exact(a.activation,b.activation,half:true)
        let down=try absolute(a.output,b.output)
        let reference=try oracle(a.activation,weights.down,a.output,b.output,b.partials!,n:n,m:4096,acc:acc,all:false)
        try require(x.digest()==original,"weighted input changed");try weights.unchanged();try a.guards();try b.guards()
        try runner.memory("weighted_\(n)_\(acc)")
        try emit(["kind":"weighted_correctness","n":n,"accumulate":acc,"gate_values":gate,"up_values":up,"activation_values":activation,"gate_up_activation_bit_differences":0,"down":down,"f64_oracle":reference,"passed":true,"guards_and_inputs_unchanged":true])
    }
}
func timed(_ runner:Runner,_ weights:Weights,n:Int)throws {
    try autoreleasepool {
        let x=try Buffer(runner.device,n*4096*2);fillHalf(x,width:4096,seed:19,scale:1)
        let initial=x.digest(),baseline=try State(runner.device,n,split:false),state=try State(runner.device,n,split:true)
        baseline.reset();_=try runner.ffn(false,weights,x,baseline,n:n)
        let hashes=baseline.buffers.map{$0.digest()}
        for round in 0..<5 {for (slot,candidate) in [false,true,true,false].enumerated() {
            for scope in ["gate","up","down_accumulate","full_ffn_accumulate"] {
                state.reset();let timing:Timing
                if scope=="full_ffn_accumulate" {timing=try runner.ffn(candidate,weights,x,state,n:n)}
                else {timing=try runner.submit {e in
                    if scope=="gate" {try runner.gemm(e,false,x:x,w:weights.gate,y:state.gate,n:n,m:12288,k:4096)}
                    else if scope=="up" {try runner.gemm(e,false,x:x,w:weights.up,y:state.up,n:n,m:12288,k:4096)}
                    else {try runner.gemm(e,candidate,x:baseline.activation,w:weights.down,y:state.output,n:n,m:4096,k:12288,acc:true,partials:state.partials)}
                }}
                try state.guards();var down:[String:Any]=[:]
                if scope=="gate" {_=try exact(baseline.gate,state.gate)}
                else if scope=="up" {_=try exact(baseline.up,state.up)}
                else {
                    if scope=="full_ffn_accumulate" {_=try exact(baseline.gate,state.gate);_=try exact(baseline.up,state.up);_=try exact(baseline.activation,state.activation,half:true)}
                    down=try absolute(baseline.output,state.output)
                }
                try emit(["kind":"timing","variant":candidate ? "split3":"baseline","n":n,"round":round,"slot":slot,"scope":scope,"gpu_ms":timing.gpu*1000,"wall_ms":timing.wall*1000,"down":down,"output_gate_passed":true,"load_average":loadAverage(),"current_allocated_bytes":runner.device.currentAllocatedSize])
            }
        }}
        try require(x.digest()==initial && baseline.buffers.map{$0.digest()}==hashes,"timed input or immutable expected output changed")
        try weights.unchanged();try runner.memory("timed_\(n)")
    }
}
do {
    for (n,m,k) in [(0,1,12288),(2049,1,12288),(1,0,12288),(1,12289,12288),(1,1,4096),(1,1,12287),(1,1,12289)] {
        var refused=false;do {try Partials.validate(n:n,m:m,k:k)}catch{refused=true};try require(refused,"invalid split shape accepted")
    }
    let runner=try Runner()
    for (n,m,kind) in smallCases {for acc in [false,true] {try small(runner,n:n,m:m,kind:kind,acc:acc)}}
    let weights=try Weights(runner.device);try runner.memory("weights_loaded")
    for n in [65,512,1024,2048] {try weighted(runner,weights,n:n,acc:true)}
    try weighted(runner,weights,n:65,acc:false)
    try emit(["kind":"correctness_complete","small_checks":14,"weighted_checks":5,"fixed_absolute_error_bound":0.001])
    if benchmark {for n in [512,1024,2048] {try timed(runner,weights,n:n)}}
    try require(commands==(benchmark ? 281:38) && dispatches==(benchmark ? 579:87),"dispatch or command count differs from frozen plan")
    try emit(["kind":"complete","passed":true,"timing_included":benchmark,"commands":commands,"dispatches":dispatches,"explicit_peak_bytes":peakBytes,"current_allocated_bytes":runner.device.currentAllocatedSize,"seconds":ProcessInfo.processInfo.systemUptime-started,"full_model_loaded":false])
}catch {
    try? emit(["kind":"failure","error":String(describing:error),"commands":commands,"dispatches":dispatches,"seconds":ProcessInfo.processInfo.systemUptime-started]);exit(1)
}
