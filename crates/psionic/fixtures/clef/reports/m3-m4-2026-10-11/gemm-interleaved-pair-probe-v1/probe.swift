// One interleaved full-K gate/up product; separate f32-to-half SiLU. No model loading.
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
func kernel(_ m: Int, _ k: Int, _ accumulate: Bool) -> String {
    let t = tile(m, k)
    return (t == 64 ? "clef_gemm" : "clef_gemm_64x\(t)") + (accumulate ? "_accumulate" : "")
}
func packedColumns(_ f: Int) -> Int { ((f + 127) / 128) * 256 }
func packedColumn(_ column: Int, up: Bool) -> Int { (column / 128) * 256 + column % 128 + (up ? 128 : 0) }
func validateShape(n: Int, h: Int, f: Int) throws {
    try require(n > 0 && n <= 2048 && h > 0 && h <= 12288 && f > 0 && f <= 12288, "shape exceeds frozen probe bounds")
}
final class State {
    let n, h, f: Int
    var paired: Bool
    let projection, activation, output: Buffer
    var buffers: [Buffer] { [projection, activation, output] }
    init(_ device: MTLDevice, n: Int, h: Int, f: Int, paired: Bool) throws {
        self.n=n; self.h=h; self.f=f; self.paired=paired
        projection = try Buffer(device, n * (paired ? packedColumns(f) : 2*f) * 4)
        activation = try Buffer(device, n*f*2); output = try Buffer(device, n*h*4)
    }
    func index(_ row: Int, _ column: Int, up: Bool) -> Int {
        paired ? row*packedColumns(f)+packedColumn(column,up:up) : (up ? n*f : 0)+row*f+column
    }
    func reset(_ acc: Bool) {
        projection.poisonFloat(); activation.poisonHalf()
        if acc { residual(output) } else { output.poisonFloat() }
    }
    func guards() throws { for b in buffers { try b.guards() } }
    func padding() throws {
        if paired {
            let p=projection.pointer.assumingMemoryBound(to:Float.self), end=((f+127)/128)*128
            for r in 0..<n { for c in f..<end { for u in [false,true] {
                let v=p[index(r,c,up:u)]
                try require(v.isFinite && v == 0, "zero-weight padded column was not written as finite zero")
            }}}
        }
    }
}
struct Timing { let gpu, wall: Double }
final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    var pipelines=[String:MTLComputePipelineState]()
    init() throws {
        guard let d=MTLCreateSystemDefaultDevice(), let q=d.makeCommandQueue() else { throw Failure("Metal unavailable") }
        device=d; queue=q
        let library=try d.makeLibrary(URL:root.appendingPathComponent("kernels.metallib"))
        for name in ["clef_gemm","clef_gemm_accumulate","clef_gemm_64x32","clef_gemm_64x32_accumulate","clef_gemm_64x128","clef_gemm_64x128_accumulate","clef_silu_mul_to_f16","interleaved_gate_up","interleaved_silu"] {
            guard let function=library.makeFunction(name:name) else { throw Failure("missing kernel \(name)") }
            let p=try d.makeComputePipelineState(function:function)
            try require(p.threadExecutionWidth == 32 && p.maxTotalThreadsPerThreadgroup >= 128 && p.staticThreadgroupMemoryLength <= d.maxThreadgroupMemoryLength,"pipeline limits unsupported")
            pipelines[name]=p
        }
        try emit(["kind":"device","name":d.name,"current_allocated_bytes":d.currentAllocatedSize,"explicit_limit_bytes":dataLimit,"combined_limit_bytes":driverLimit,"load_average":loadAverage()])
    }
    func memory(_ stage: String) throws {
        try memoryCheck(device)
        try emit(["kind":"allocation","stage":stage,"current_allocated_bytes":device.currentAllocatedSize,"explicit_live_bytes":liveBytes,"owned_expected_bytes":expectedBytes,"combined_bytes":device.currentAllocatedSize+expectedBytes+transientAssetBytes])
    }
    func gemm(_ e: MTLComputeCommandEncoder, x: Buffer, w: Buffer, y: Buffer, n: Int, m: Int, k: Int, offset: Int=0, acc: Bool=false) throws {
        try validateShape(n:n,h:k,f:m)
        try require(x.bytes==n*k*2 && w.bytes==m*k*2 && offset>=0 && offset+n*m*4<=y.bytes,"GEMM binding bounds differ")
        e.memoryBarrier(scope:.buffers); e.setComputePipelineState(pipelines[kernel(m,k,acc)]!)
        e.setBuffer(x.metal,offset:pad,index:0);e.setBuffer(w.metal,offset:pad,index:1);e.setBuffer(y.metal,offset:pad+offset,index:2)
        var a=SIMD4<UInt32>(UInt32(n),UInt32(m),UInt32(k),0);e.setBytes(&a,length:16,index:3)
        e.dispatchThreadgroups(MTLSize(width:(m+tile(m,k)-1)/tile(m,k),height:(n+63)/64,depth:1),threadsPerThreadgroup:MTLSize(width:128,height:1,depth:1));dispatches+=1
    }
    func gateUp(_ e: MTLComputeCommandEncoder,_ w: Weights,_ x: Buffer,_ s: State) throws {
        try validateShape(n:s.n,h:s.h,f:s.f)
        try require(w.h==s.h && w.f==s.f && x.bytes==s.n*s.h*2,"gate/up dimensions differ")
        if s.paired {
            try require(w.packed.bytes==packedColumns(s.f)*s.h*2 && s.projection.bytes==s.n*packedColumns(s.f)*4,"paired binding lengths differ")
            e.memoryBarrier(scope:.buffers);e.setComputePipelineState(pipelines["interleaved_gate_up"]!)
            for (i,b) in [x,w.packed,s.projection].enumerated(){e.setBuffer(b.metal,offset:pad,index:i)}
            var a=SIMD4<UInt32>(UInt32(s.n),UInt32(s.f),UInt32(s.h),UInt32(packedColumns(s.f)));e.setBytes(&a,length:16,index:3)
            e.dispatchThreadgroups(MTLSize(width:packedColumns(s.f)/256,height:(s.n+63)/64,depth:1),threadsPerThreadgroup:MTLSize(width:128,height:1,depth:1));dispatches+=1
        } else {
            try gemm(e,x:x,w:w.gate,y:s.projection,n:s.n,m:s.f,k:s.h)
            try gemm(e,x:x,w:w.up,y:s.projection,n:s.n,m:s.f,k:s.h,offset:s.n*s.f*4)
        }
    }
    func silu(_ e: MTLComputeCommandEncoder,_ s: State) {
        e.memoryBarrier(scope:.buffers)
        if s.paired {
            e.setComputePipelineState(pipelines["interleaved_silu"]!)
            e.setBuffer(s.projection.metal,offset:pad,index:0);e.setBuffer(s.activation.metal,offset:pad,index:1)
            var a=SIMD4<UInt32>(UInt32(s.n),UInt32(s.f),UInt32(s.h),UInt32(packedColumns(s.f)));e.setBytes(&a,length:16,index:2)
        } else {
            e.setComputePipelineState(pipelines["clef_silu_mul_to_f16"]!)
            e.setBuffer(s.projection.metal,offset:pad,index:0);e.setBuffer(s.projection.metal,offset:pad+s.n*s.f*4,index:1);e.setBuffer(s.activation.metal,offset:pad,index:2)
            var count=UInt64(s.n*s.f);e.setBytes(&count,length:8,index:3)
        }
        e.dispatchThreadgroups(MTLSize(width:(s.n*s.f+127)/128,height:1,depth:1),threadsPerThreadgroup:MTLSize(width:128,height:1,depth:1));dispatches+=1
    }
    func submit(_ encode:(MTLComputeCommandEncoder)throws->Void) throws -> Timing {
        try timeCheck()
        return try autoreleasepool {
            let start=ProcessInfo.processInfo.systemUptime
            guard let cb=queue.makeCommandBuffer(),let e=cb.makeComputeCommandEncoder(dispatchType:.concurrent) else { throw Failure("command allocation failed") }
            try encode(e);e.endEncoding();cb.commit();cb.waitUntilCompleted();commands+=1
            try require(cb.status == .completed,"GPU command failed: \(String(describing:cb.error))")
            try memoryCheck(device)
            let gpu=cb.gpuEndTime-cb.gpuStartTime
            try require(gpu.isFinite && gpu>0,"invalid GPU timestamps")
            return Timing(gpu:gpu,wall:ProcessInfo.processInfo.systemUptime-start)
        }
    }
    func ffn(_ w: Weights,_ x: Buffer,_ s: State,acc: Bool) throws -> Timing {
        try submit{e in
            try gateUp(e,w,x,s);silu(e,s)
            try gemm(e,x:s.activation,w:w.down,y:s.output,n:s.n,m:s.h,k:s.f,acc:acc)
        }
    }
}
final class Weights {
    let h,f:Int
    let gate,up,down,packed:Buffer
    let hashes:[String]
    var buffers:[Buffer]{[gate,up,down,packed]}
    init(_ device:MTLDevice,h:Int,f:Int,synthetic:Bool) throws {
        self.h=h;self.f=f
        gate=try Buffer(device,h*f*2);up=try Buffer(device,h*f*2);down=try Buffer(device,h*f*2)
        if synthetic {
            fillHalf(gate,width:h,seed:41,scale:1/sqrt(Float(h)))
            fillHalf(up,width:h,seed:67,scale:1/sqrt(Float(h)))
            fillHalf(down,width:f,seed:113,scale:1/sqrt(Float(f)))
        } else {
            try require(h==4096 && f==12288,"real-weight dimensions differ")
            let manifest=try JSONSerialization.jsonObject(with:Data(contentsOf:root.appendingPathComponent("weights.json"))) as! [String:Any]
            let entries=manifest["tensors"] as! [[String:Any]]
            for (file,b) in [("gate.f16",gate),("up.f16",up),("down.f16",down)] {
                try autoreleasepool {
                    transientAssetBytes+=b.bytes;defer{transientAssetBytes-=b.bytes};try memoryCheck(device)
                    let entry=entries.first{$0["half_file"] as? String == file}!
                    let data=try Data(contentsOf:assets.appendingPathComponent(file),options:.mappedIfSafe)
                    try require(data.count==b.bytes && hash(data)==entry["half_sha256"] as! String,"real weight hash or size differs")
                    data.withUnsafeBytes{b.pointer.copyMemory(from:$0.baseAddress!,byteCount:b.bytes)}
                }
            }
            try emit(["kind":"weights","manifest":manifest,"full_model_loaded":false])
        }
        let began=ProcessInfo.processInfo.systemUptime
        packed=try Buffer(device,packedColumns(f)*h*2)
        packed.pointer.initializeMemory(as:UInt8.self,repeating:0,count:packed.bytes)
        for c in 0..<f {for (isUp,source) in [(false,gate),(true,up)] {
            packed.pointer.advanced(by:packedColumn(c,up:isUp)*h*2).copyMemory(from:source.pointer.advanced(by:c*h*2),byteCount:h*2)
        }}
        for p in 0..<packedColumns(f) {
            let c=(p/256)*128+p%128,isUp=p%256>=128
            let target=packed.pointer.advanced(by:p*h*2)
            if c<f {
                let source=(isUp ? up:gate).pointer.advanced(by:c*h*2)
                try require(memcmp(target,source,h*2)==0,"packed row differs from source")
            } else {
                let values=target.assumingMemoryBound(to:UInt16.self)
                for k in 0..<h {try require(values[k]==0,"padded weight row is not zero")}
            }
        }
        hashes=[gate,up,down,packed].map{$0.digest()}
        try emit(["kind":"packing","h":h,"f":f,"packed_rows":packedColumns(f),"validated_half_values":packedColumns(f)*h,"zero_rows":packedColumns(f)-2*f,"seconds":ProcessInfo.processInfo.systemUptime-began,"paired_sha256":hashes[3],"passed":true])
    }
    func unchanged() throws {
        for (b,h) in zip(buffers,hashes){try b.guards();try require(b.digest()==h,"immutable weight changed")}
    }
}
// Own the saved bytes independently of every Metal workspace.
final class Expected {
    let n,h,f:Int,paired:Bool
    let values:[Data],hashes:[String],bytes:Int
    init(_ device:MTLDevice,_ state:State) throws {
        n=state.n;h=state.h;f=state.f;paired=state.paired
        bytes=state.buffers.reduce(0){$0+$1.bytes}
        try require(device.currentAllocatedSize+expectedBytes+bytes<=driverLimit,"immutable expectation allocation exceeds combined limit")
        values=state.buffers.map{Data(bytes:$0.pointer,count:$0.bytes)}
        hashes=values.map{hash($0)}
        expectedBytes+=bytes;peakExpectedBytes=max(peakExpectedBytes,expectedBytes);try memoryCheck(device)
    }
    deinit{expectedBytes-=bytes}
    func unchanged() throws{try require(values.map{hash($0)}==hashes,"immutable expected bytes changed")}
    func check(_ index:Int,_ output:Buffer) throws -> [String:Any] {
        try output.guards();let data=values[index]
        try require(data.count==output.bytes,"repeat length differs")
        let equal=data.withUnsafeBytes{memcmp($0.baseAddress!,output.pointer,output.bytes)==0}
        if !equal {
            try emit(["kind":"repeat_failure","index":index,"expected_sha256":hashes[index],"actual_sha256":hash(Data(bytesNoCopy:output.pointer,count:output.bytes,deallocator:.none))])
            throw Failure("timed output differs from its fully checked expectation")
        }
        return ["bytes":output.bytes,"bitwise_equal":true,"passed":true]
    }
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
func projectionIndex(_ paired:Bool,_ n:Int,_ f:Int,_ row:Int,_ col:Int,_ up:Bool)->Int {
    paired ? row*packedColumns(f)+packedColumn(col,up:up):(up ? n*f:0)+row*f+col
}
func compareField(_ expected:Expected,_ actual:State,_ field:String)throws->[String:Any] {
    let half=field=="activation",projection=field=="gate" || field=="up"
    let count=actual.n*(field=="down" ? actual.h:actual.f),bufferIndex=projection ? 0:(half ? 1:2)
    var maximum=0.0,differences=0,aboveAbsolute=0,aboveULP=0,maxULP=0,violations=0
    var first=[[String:Any]]()
    let actualPointer=actual.buffers[bufferIndex].pointer
    try expected.values[bufferIndex].withUnsafeBytes{raw in
        for i in 0..<count {
            let ai=projection ? projectionIndex(expected.paired,actual.n,actual.f,i/actual.f,i%actual.f,field=="up"):i
            let bi=projection ? actual.index(i/actual.f,i%actual.f,up:field=="up"):i
            let av:Double,bv:Double,ab:UInt32,bb:UInt32
            if half {
                let a=raw.baseAddress!.assumingMemoryBound(to:Float16.self)[ai],b=actualPointer.assumingMemoryBound(to:Float16.self)[bi]
                av=Double(a);bv=Double(b);ab=UInt32(a.bitPattern);bb=UInt32(b.bitPattern)
            }else{
                let a=raw.baseAddress!.assumingMemoryBound(to:Float.self)[ai],b=actualPointer.assumingMemoryBound(to:Float.self)[bi]
                av=Double(a);bv=Double(b);ab=a.bitPattern;bb=b.bitPattern
            }
            try require(av.isFinite && bv.isFinite,"nonfinite or unwritten output value")
            let error=abs(av-bv),ulp=half ? halfULP(UInt16(ab),UInt16(bb)):0
            maximum=max(maximum,error);maxULP=max(maxULP,ulp)
            if ab != bb {differences+=1};if error>0.001 {aboveAbsolute+=1};if half && ulp>1 {aboveULP+=1}
            if error>0.001 && (!half || ulp>1) {
                violations+=1
                if first.count<8 {first.append(["index":i,"baseline":av,"candidate":bv,"error":error,"half_ulp_distance":ulp])}
            }
        }
    }
    let result:[String:Any]=["values":count,"half":half,"max_abs":maximum,"bit_differences":differences,"above_0_001":aboveAbsolute,"half_max_ulp":maxULP,"half_above_one_ulp":aboveULP,"criterion_violations":violations,"criterion_version":2,"first_failures":first,"passed":violations==0]
    if violations>0 {try emit(["kind":"numerical_failure","field":field,"comparison":result]);throw Failure("frozen numerical gate failed")}
    return result
}
func finiteState(_ s:State)throws {
    for (index,b) in s.buffers.enumerated(){
        if index==1 {let p=b.pointer.assumingMemoryBound(to:Float16.self);for i in 0..<(b.bytes/2){try require(p[i].isFinite,"nonfinite half expectation")}}
        else{let p=b.pointer.assumingMemoryBound(to:Float.self);for i in 0..<(b.bytes/4){try require(p[i].isFinite,"nonfinite f32 expectation")}}
    }
    try s.guards();try s.padding()
}
func residualValue(_ i:Int)->Float{Float((i*13+17)%101-50)/37}
func oracle(_ weights:Weights,_ x:Buffer,_ a:Expected,_ b:State,field:String,acc:Bool,all:Bool)throws->[String:Any] {
    let down=field=="down",m=down ? b.h:b.f,k=down ? b.f:b.h
    let weight=down ? weights.down:(field=="gate" ? weights.gate:weights.up)
    let wp=weight.pointer.assumingMemoryBound(to:Float16.self),xp=x.pointer.assumingMemoryBound(to:Float16.self)
    let indices=all ? Array(0..<(b.n*m)):(0..<96).map{s in s<4 ? (s%2==0 ? 0:b.n-1)*m+(s<2 ? 0:m-1):((s*97+5)%b.n)*m+(s*997+11)%m}
    var maxA=0.0,maxB=0.0
    try a.values[down ? 2:0].withUnsafeBytes{outRaw in
        try a.values[1].withUnsafeBytes{actRaw in
            let ap=outRaw.baseAddress!.assumingMemoryBound(to:Float.self),ac=actRaw.baseAddress!.assumingMemoryBound(to:Float16.self)
            let bp=b.buffers[down ? 2:0].pointer.assumingMemoryBound(to:Float.self),bc=b.activation.pointer.assumingMemoryBound(to:Float16.self)
            for index in indices {
                let row=index/m,col=index%m
                var ea=0.0,eb=0.0
                for j in 0..<k {
                    let w=Double(wp[col*k+j]);ea+=Double(down ? ac[row*k+j]:xp[row*k+j])*w;eb+=Double(down ? bc[row*k+j]:xp[row*k+j])*w
                }
                if down && acc {ea+=Double(residualValue(index));eb+=Double(residualValue(index))}
                let ai=down ? index:projectionIndex(a.paired,b.n,b.f,row,col,field=="up"),bi=down ? index:b.index(row,col,up:field=="up")
                let da=abs(Double(ap[ai])-ea),db=abs(Double(bp[bi])-eb)
                if !(ea.isFinite && eb.isFinite && ap[ai].isFinite && bp[bi].isFinite && da<=0.001 && db<=0.001) {
                    try emit(["kind":"oracle_failure","field":field,"row":row,"column":col,"baseline_error_text":String(da),"candidate_error_text":String(db)]);throw Failure("independent f64 gate failed")
                }
                maxA=max(maxA,da);maxB=max(maxB,db)
            }
        }
    }
    return ["sample_count":indices.count,"all_outputs":all,"baseline_max_abs":maxA,"candidate_max_abs":maxB,"passed":true]
}
let smallCases:[(Int,Int,Int,String)]=[(1,1,1,"zero"),(3,7,127,"nonsymmetric"),(7,129,257,"correlated"),(63,1,4096,"cancellation"),(64,128,65,"mixedrange"),(65,129,257,"nonsymmetric"),(257,1,12288,"nonsymmetric"),(1,33,4096,"mixedrange"),(33,1,12288,"cancellation")]
func fillInput(_ x:Buffer,h:Int,kind:String){
    fillHalf(x,width:h,seed:19,scale:kind=="weighted" ? 1:0.125)
    let p=x.pointer.assumingMemoryBound(to:Float16.self)
    if kind=="zero" {p.initialize(repeating:0,count:x.bytes/2)}
    if kind=="correlated" {for i in 0..<(x.bytes/2){p[i]=Float16(Float(i%17-8)/64)}}
    if kind=="cancellation" {for i in 0..<(x.bytes/2){p[i]=Float16(i%2==0 ? 0.25:-0.25)}}
    if kind=="mixedrange" {for i in 0..<(x.bytes/2){p[i]=Float16(i%29==0 ? 4:Float(i%7-3)/1024)}}
}
func baselineExpected(_ r:Runner,_ w:Weights,_ x:Buffer,n:Int,acc:Bool)throws->Expected{
    try autoreleasepool{
        let state=try State(r.device,n:n,h:w.h,f:w.f,paired:false);state.reset(acc)
        _=try r.ffn(w,x,state,acc:acc);try finiteState(state)
        return try Expected(r.device,state)
    }
}
func checkCase(_ r:Runner,_ w:Weights,n:Int,acc:Bool,kind:String,weighted:Bool)throws{
    try autoreleasepool{
        let x=try Buffer(r.device,n*w.h*2);fillInput(x,h:w.h,kind:kind);let initial=x.digest()
        let baseline=try baselineExpected(r,w,x,n:n,acc:acc)
        try require(liveBytes==(w.buffers+[x]).reduce(0){$0+$1.bytes+2*pad},"baseline workspace did not release")
        let candidate=try State(r.device,n:n,h:w.h,f:w.f,paired:true);candidate.reset(acc)
        _=try r.ffn(w,x,candidate,acc:acc);try finiteState(candidate)
        var comparisons=[String:[String:Any]](),oracles=[String:[String:Any]]()
        for field in ["gate","up","activation","down"]{comparisons[field]=try compareField(baseline,candidate,field)}
        for field in ["gate","up","down"]{oracles[field]=try oracle(w,x,baseline,candidate,field:field,acc:acc,all:!weighted)}
        try require(x.digest()==initial,"input changed");try x.guards();try w.unchanged();try baseline.unchanged();try r.memory("checked_\(n)_\(w.f)_\(w.h)")
        try emit(["kind":weighted ? "weighted_correctness":"small_correctness","n":n,"m":w.f,"k":w.h,"fixture":kind,"accumulate":acc,"comparisons":comparisons,"f64_oracles":oracles,"padding_checked":true,"guards_and_inputs_unchanged":true,"passed":true])
    }
}
func expectations(_ r:Runner,_ w:Weights,_ x:Buffer,n:Int)throws->(Expected,Expected){
    let baseline=try baselineExpected(r,w,x,n:n,acc:true)
    let candidate:Expected=try autoreleasepool{
        try require(liveBytes==(w.buffers+[x]).reduce(0){$0+$1.bytes+2*pad},"baseline state still allocated")
        let state=try State(r.device,n:n,h:w.h,f:w.f,paired:true);state.reset(true)
        _=try r.ffn(w,x,state,acc:true);try finiteState(state)
        var comparisons=[String:[String:Any]](),oracles=[String:[String:Any]]()
        for field in ["gate","up","activation","down"]{comparisons[field]=try compareField(baseline,state,field)}
        for field in ["gate","up","down"]{oracles[field]=try oracle(w,x,baseline,state,field:field,acc:true,all:false)}
        let saved=try Expected(r.device,state)
        try emit(["kind":"expectations_checked","n":n,"comparisons":comparisons,"f64_oracles":oracles,"baseline_sha256":baseline.hashes,"candidate_sha256":saved.hashes,"passed":true])
        return saved
    }
    return(baseline,candidate)
}
func timed(_ r:Runner,_ w:Weights,n:Int)throws{
    try autoreleasepool{
        let x=try Buffer(r.device,n*w.h*2);fillInput(x,h:w.h,kind:"weighted");let initial=x.digest()
        let (baseline,candidate)=try expectations(r,w,x,n:n)
        try require(liveBytes==(w.buffers+[x]).reduce(0){$0+$1.bytes+2*pad},"expectation workspace did not release")
        try r.memory("expectations_frozen_\(n)")
        try require(2*w.f==packedColumns(w.f),"timing workspace cannot share differently sized layouts")
        let state=try State(r.device,n:n,h:w.h,f:w.f,paired:false)
        let commonActivation=try Buffer(r.device,n*w.f*2)
        baseline.values[1].withUnsafeBytes{commonActivation.pointer.copyMemory(from:$0.baseAddress!,byteCount:commonActivation.bytes)}
        let commonHash=commonActivation.digest()
        for round in 0..<5{for(slot,paired)in [false,true,true,false].enumerated(){
            state.paired=paired
            let expected=paired ? candidate:baseline
            for scope in ["gate_up","activation","down_accumulate","full_ffn_accumulate"]{
                state.reset(true)
                if scope=="activation"{expected.values[0].withUnsafeBytes{state.projection.pointer.copyMemory(from:$0.baseAddress!,byteCount:state.projection.bytes)}}
                let timing:Timing
                if scope=="full_ffn_accumulate"{timing=try r.ffn(w,x,state,acc:true)}
                else{timing=try r.submit{e in
                    if scope=="gate_up"{try r.gateUp(e,w,x,state)}
                    else if scope=="activation"{r.silu(e,state)}
                    else{try r.gemm(e,x:commonActivation,w:w.down,y:state.output,n:n,m:w.h,k:w.f,acc:true)}
                }}
                try state.guards();try commonActivation.guards()
                var repeats=[String:[String:Any]]()
                if scope=="gate_up"{repeats["projections"]=try expected.check(0,state.projection)}
                else if scope=="activation"{
                    repeats["activation"]=try expected.check(1,state.activation)
                    repeats["source_projections"]=try expected.check(0,state.projection)
                }else if scope=="down_accumulate"{repeats["down"]=try baseline.check(2,state.output)}
                else{for(i,name)in ["projections","activation","down"].enumerated(){repeats[name]=try expected.check(i,state.buffers[i])}}
                try emit(["kind":"timing","n":n,"round":round,"slot":slot,"scope":scope,"variant":paired ? "interleaved":"baseline","gpu_ms":timing.gpu*1000,"wall_ms":timing.wall*1000,"output_gate_passed":true,"repeats":repeats,"load_average":loadAverage(),"current_allocated_bytes":r.device.currentAllocatedSize,"owned_expected_bytes":expectedBytes])
            }
        }}
        try require(x.digest()==initial && commonActivation.digest()==commonHash,"timed source changed")
        try baseline.unchanged();try candidate.unchanged();try w.unchanged();try r.memory("timed_\(n)")
    }
}
do{
    for(n,h,f)in [(0,1,1),(2049,1,1),(1,0,1),(1,12289,1),(1,1,0),(1,1,12289)]{
        var refused=false;do{try validateShape(n:n,h:h,f:f)}catch{refused=true};try require(refused,"invalid shape accepted")
    }
    try validateULPHelper()
    let runner=try Runner()
    for(n,f,h,kind)in smallCases{
        try autoreleasepool{
            let w=try Weights(runner.device,h:h,f:f,synthetic:true)
            for acc in [false,true]{try checkCase(runner,w,n:n,acc:acc,kind:kind,weighted:false)}
        }
    }
    let weights=try Weights(runner.device,h:4096,f:12288,synthetic:false);try runner.memory("weights_loaded")
    for n in [65,512,1024,2048]{try checkCase(runner,weights,n:n,acc:true,kind:"weighted",weighted:true)}
    try checkCase(runner,weights,n:65,acc:false,kind:"weighted",weighted:true)
    try emit(["kind":"correctness_complete","small_checks":18,"weighted_checks":5,"fixed_f32_absolute_error_bound":0.001,"half_activation_absolute_or_ulp_bound":[0.001,1],"criterion_version":2])
    if benchmark{for n in [512,1024,2048]{try timed(runner,weights,n:n)}}
    try require(commands==(benchmark ? 292:46) && dispatches==(benchmark ? 602:161),"command/dispatch count differs from frozen plan")
    try emit(["kind":"complete","passed":true,"timing_included":benchmark,"commands":commands,"dispatches":dispatches,"explicit_peak_bytes":peakBytes,"expected_peak_bytes":peakExpectedBytes,"combined_driver_and_expected_peak_bytes":peakCombinedBytes,"seconds":ProcessInfo.processInfo.systemUptime-started,"full_model_loaded":false])
}catch{
    try? emit(["kind":"failure","error":String(describing:error),"commands":commands,"dispatches":dispatches,"seconds":ProcessInfo.processInfo.systemUptime-started]);exit(1)
}
