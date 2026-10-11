// Bounded kernel experiment with one real Flash FFN layer and deterministic inputs.
import Foundation
import Metal
import CryptoKit
import Darwin

struct Failure: Error, CustomStringConvertible {
    let description: String
    init(_ value: String) { description = value }
}
func require(_ valid: Bool, _ message: String) throws { if !valid { throw Failure(message) } }
func emit(_ item: [String: Any]) throws {
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: item, options: [.sortedKeys]))
    FileHandle.standardOutput.write(Data([10]))
}
func loadAverage() -> [Double] {
    var values = [Double](repeating: 0, count: 3)
    let count = values.withUnsafeMutableBufferPointer { getloadavg($0.baseAddress!, 3) }
    return count == 3 ? values : []
}
struct Variant {
    let rows: Int, columns: Int, groups: Int
    var name: String { rows == 64 && columns == 64 && groups == 4 ? "clef_gemm" : "tile_\(rows)_\(columns)_\(groups)" }
}
let variants = [Variant(rows:64,columns:64,groups:4), Variant(rows:64,columns:128,groups:4),
                Variant(rows:128,columns:128,groups:4), Variant(rows:64,columns:128,groups:8),
                Variant(rows:128,columns:128,groups:8), Variant(rows:128,columns:64,groups:8),
                Variant(rows:128,columns:256,groups:8)]
let args = CommandLine.arguments
try require(args.count == 3 && ["check", "bench"].contains(args[2]), "Usage: probe DIRECTORY check|bench")
let root = URL(fileURLWithPath:args[1]), checking = args[2] == "check"
let redzone = 64, poison = Float(bitPattern:0x7fc00555)
let maxBytes = 1024 * 1024 * 1024
func allocate(_ device: MTLDevice, _ length: Int) throws -> MTLBuffer {
    try require(length > 0 && length <= maxBytes, "buffer exceeds bound")
    guard let buffer = device.makeBuffer(length:length, options:.storageModeShared) else { throw Failure("allocation failed") }
    return buffer
}
func fillHalf(_ buffer: MTLBuffer, count: Int, width: Int, seed: Int, scale: Float) {
    let p = buffer.contents().bindMemory(to:Float16.self,capacity:count)
    for i in 0..<count { p[i] = Float16(Float((i * 31 + (i / width) * 17 + seed) % 251 - 125) / 125 * scale) }
}
func resetFloat(_ buffer: MTLBuffer, _ count: Int) {
    buffer.contents().bindMemory(to:Float.self,capacity:count+2*redzone).initialize(repeating:poison,count:count+2*redzone)
}
func redzones(_ buffer: MTLBuffer, _ count: Int) throws {
    let p=buffer.contents().bindMemory(to:Float.self,capacity:count+2*redzone)
    for i in 0..<redzone { try require(p[i].bitPattern == poison.bitPattern && p[redzone+count+i].bitPattern == poison.bitPattern,"output redzone changed") }
}
func compareFloat(_ reference: MTLBuffer, _ candidate: MTLBuffer, count: Int, offset: Int = 0) throws -> [String:Any] {
    let a=reference.contents().bindMemory(to:Float.self,capacity:count+offset), b=candidate.contents().bindMemory(to:Float.self,capacity:count+offset)
    var absolute=0.0, scaled=0.0, differences=0
    for i in 0..<count {
        let x=a[i+offset],y=b[i+offset]
        try require(x.isFinite && y.isFinite,"nonfinite or unwritten output")
        let d=abs(Double(x)-Double(y));absolute=max(absolute,d);scaled=max(scaled,d/max(1,abs(Double(x))))
        if x.bitPattern != y.bitPattern { differences += 1 }
    }
    return ["values":count,"bit_differences":differences,"max_abs":absolute,"max_scaled_abs":scaled,"passed":scaled <= 0.00003,"finite":true]
}
struct Timing { let gpu:Double, wall:Double }
final class Runner {
    let device: MTLDevice, queue: MTLCommandQueue
    var pipelines=[String:MTLComputePipelineState]()
    init() throws {
        guard let d=MTLCreateSystemDefaultDevice(),let q=d.makeCommandQueue() else {throw Failure("Metal unavailable")}
        device=d;queue=q
        let source=try String(contentsOf:root.appendingPathComponent("tiles.metal"),encoding:.utf8)
        let library=try d.makeLibrary(source:source,options:MTLCompileOptions())
        for variant in variants {
            guard let function=library.makeFunction(name:variant.name) else { throw Failure("kernel missing") }
            let pipeline=try d.makeComputePipelineState(function:function)
            try require(pipeline.threadExecutionWidth == 32 && pipeline.maxTotalThreadsPerThreadgroup >= variant.groups*32,"threadgroup unsupported")
            pipelines[variant.name]=pipeline
            try emit(["kind":"pipeline","name":variant.name,"tile_rows":variant.rows,"tile_columns":variant.columns,"simdgroups":variant.groups,"static_threadgroup_bytes":pipeline.staticThreadgroupMemoryLength,"maximum_threads":pipeline.maxTotalThreadsPerThreadgroup])
        }
        guard let function=library.makeFunction(name:"probe_silu") else {throw Failure("SiLU kernel missing")}
        pipelines["probe_silu"]=try d.makeComputePipelineState(function:function)
        try emit(["kind":"device","name":d.name,"recommended_max_working_set":d.recommendedMaxWorkingSetSize,"fixture_byte_bound":maxBytes,"load_average":loadAverage(),"conditions":"shared machine; one FFN layer; no model execution"])
    }
    func gemm(_ encoder: MTLComputeCommandEncoder, _ variant: Variant, x:MTLBuffer,w:MTLBuffer,y:MTLBuffer,n:Int,m:Int,k:Int,offset:Int=0) {
        encoder.memoryBarrier(scope:.buffers)
        encoder.setComputePipelineState(pipelines[variant.name]!)
        encoder.setBuffer(x,offset:0,index:0);encoder.setBuffer(w,offset:0,index:1);encoder.setBuffer(y,offset:offset,index:2)
        var dims=SIMD4<UInt32>(UInt32(n),UInt32(m),UInt32(k),0)
        encoder.setBytes(&dims,length:16,index:3)
        encoder.dispatchThreadgroups(MTLSize(width:(m+variant.columns-1)/variant.columns,height:(n+variant.rows-1)/variant.rows,depth:1),threadsPerThreadgroup:MTLSize(width:variant.groups*32,height:1,depth:1))
    }
    func submit(_ encode: (MTLComputeCommandEncoder)->Void) throws -> Timing {
        try autoreleasepool {
            let began=ProcessInfo.processInfo.systemUptime
            guard let command=queue.makeCommandBuffer(),let encoder=command.makeComputeCommandEncoder(dispatchType:.concurrent) else {throw Failure("command allocation failed")}
            encode(encoder);encoder.endEncoding();command.commit();command.waitUntilCompleted()
            try require(command.status == .completed,"GPU command failed: \(String(describing:command.error))")
            let gpu=command.gpuEndTime-command.gpuStartTime
            try require(gpu.isFinite && gpu > 0,"invalid GPU timestamp")
            return Timing(gpu:gpu,wall:ProcessInfo.processInfo.systemUptime-began)
        }
    }
    func ffn(_ v:Variant,_ weights:Weights,_ x:MTLBuffer,_ state:State,n:Int) throws -> Timing {
        try submit { e in
            gemm(e,v,x:x,w:weights.gate,y:state.gate,n:n,m:12288,k:4096)
            gemm(e,v,x:x,w:weights.up,y:state.up,n:n,m:12288,k:4096)
            e.memoryBarrier(scope:.buffers);e.setComputePipelineState(pipelines["probe_silu"]!)
            e.setBuffer(state.gate,offset:0,index:0);e.setBuffer(state.up,offset:0,index:1);e.setBuffer(state.activation,offset:0,index:2)
            var count=UInt32(n*12288);e.setBytes(&count,length:4,index:3)
            e.dispatchThreadgroups(MTLSize(width:(Int(count)+127)/128,height:1,depth:1),threadsPerThreadgroup:MTLSize(width:128,height:1,depth:1))
            gemm(e,v,x:state.activation,w:weights.down,y:state.output,n:n,m:4096,k:12288,offset:redzone*4)
        }
    }
}
final class Weights {
    let gate:MTLBuffer,up:MTLBuffer,down:MTLBuffer
    init(_ device:MTLDevice) throws {
        let manifest=try JSONSerialization.jsonObject(with:Data(contentsOf:root.appendingPathComponent("weights.json"))) as! [String:Any]
        let entries=manifest["tensors"] as! [[String:Any]]
        func read(_ name:String) throws -> MTLBuffer {
            try autoreleasepool {
                let entry=entries.first(where:{$0["half_file"] as? String == name+".f16"})!
                let data=try Data(contentsOf:root.appendingPathComponent(name+".f16"),options:.mappedIfSafe)
                try require(data.count == 4096*12288*2,"weight size differs")
                let hash=SHA256.hash(data:data).map {String(format:"%02x",$0)}.joined()
                try require(hash == entry["half_sha256"] as! String,"staged weight hash differs")
                let result=try allocate(device,data.count)
                data.withUnsafeBytes { bytes in result.contents().copyMemory(from:bytes.baseAddress!,byteCount:data.count) }
                return result
            }
        }
        gate=try read("gate");up=try read("up");down=try read("down")
        try emit(["kind":"weights","manifest":manifest,"weights_bytes":3*4096*12288*2])
    }
}
final class State {
    let gate:MTLBuffer,up:MTLBuffer,activation:MTLBuffer,output:MTLBuffer
    init(_ device:MTLDevice,_ n:Int) throws {
        gate=try allocate(device,n*12288*4);up=try allocate(device,n*12288*4)
        activation=try allocate(device,n*12288*2);output=try allocate(device,(n*4096+2*redzone)*4)
        resetFloat(output,n*4096)
    }
}

do {
    let runner=try Runner()
    var valid=Set(variants.map(\.name))
    for (n,m,k) in [(1,1,1),(7,31,63),(63,65,129),(64,128,128),(65,129,257),(127,255,65),(129,257,63)] {
        try autoreleasepool {
            let x=try allocate(runner.device,n*k*2),w=try allocate(runner.device,m*k*2)
            let ref=try allocate(runner.device,(n*m+2*redzone)*4),out=try allocate(runner.device,(n*m+2*redzone)*4)
            fillHalf(x,count:n*k,width:k,seed:19,scale:1);fillHalf(w,count:m*k,width:k,seed:41,scale:1/sqrt(Float(k)))
            let xp=x.contents().bindMemory(to:Float16.self,capacity:n*k),wp=w.contents().bindMemory(to:Float16.self,capacity:m*k)
            var oracle=[Double](repeating:0,count:n*m)
            for row in 0..<n {for col in 0..<m {for t in 0..<k {oracle[row*m+col] += Double(xp[row*k+t])*Double(wp[col*k+t])}}}
            resetFloat(ref,n*m)
            _=try runner.submit {runner.gemm($0,variants[0],x:x,w:w,y:ref,n:n,m:m,k:k,offset:redzone*4)}
            for v in variants {
                resetFloat(out,n*m)
                _=try runner.submit {runner.gemm($0,v,x:x,w:w,y:out,n:n,m:m,k:k,offset:redzone*4)}
                try redzones(out,n*m);try redzones(ref,n*m)
                var result=try compareFloat(ref,out,count:n*m,offset:redzone)
                let p=out.contents().bindMemory(to:Float.self,capacity:n*m+2*redzone)
                var maximum=0.0,scaled=0.0
                for i in 0..<n*m {let d=abs(Double(p[i+redzone])-oracle[i]);maximum=max(maximum,d);scaled=max(scaled,d/max(1,abs(oracle[i])))}
                let passed=result["passed"] as! Bool && scaled<=0.00003
                if !passed {valid.remove(v.name)}
                result.merge(["kind":"small_correctness","name":v.name,"n":n,"m":m,"k":k,"f64_max_abs":maximum,"f64_max_scaled_abs":scaled,"passed":passed,"redzones_unchanged":true],uniquingKeysWith:{$1})
                try emit(result)
            }
        }
    }
    try require(valid.contains(variants[0].name),"baseline failed independent small oracle")
    if !checking {
        let weights=try Weights(runner.device)
        for n in [65,1024,2048] {
            try autoreleasepool {
                let bytes=3*4096*12288*2+n*4096*2+2*(n*12288*10+(n*4096+2*redzone)*4)
                try require(bytes<=maxBytes,"full fixture exceeds data bound before allocation")
                let x=try allocate(runner.device,n*4096*2)
                fillHalf(x,count:n*4096,width:4096,seed:19,scale:1)
                let reference=try State(runner.device,n),candidate=try State(runner.device,n)
                _=try runner.ffn(variants[0],weights,x,reference,n:n)
                var passing=[Variant]()
                for v in variants where valid.contains(v.name) {
                    resetFloat(candidate.output,n*4096)
                    _=try runner.ffn(v,weights,x,candidate,n:n)
                    let gate=try compareFloat(reference.gate,candidate.gate,count:n*12288),up=try compareFloat(reference.up,candidate.up,count:n*12288)
                    let down=try compareFloat(reference.output,candidate.output,count:n*4096,offset:redzone)
                    let a=reference.activation.contents().bindMemory(to:Float16.self,capacity:n*12288),b=candidate.activation.contents().bindMemory(to:Float16.self,capacity:n*12288)
                    var mismatches=0,maximum=0.0
                    for i in 0..<n*12288 {try require(a[i].isFinite && b[i].isFinite,"nonfinite activation");if a[i].bitPattern != b[i].bitPattern {mismatches+=1};maximum=max(maximum,abs(Double(a[i])-Double(b[i])))}
                    try redzones(reference.output,n*4096);try redzones(candidate.output,n*4096)
                    let passed=(gate["passed"] as! Bool)&&(up["passed"] as! Bool)&&(down["passed"] as! Bool)&&mismatches==0
                    if passed {passing.append(v)}
                    try emit(["kind":"weighted_correctness","name":v.name,"n":n,"gate":gate,"up":up,"down":down,"activation_bit_differences":mismatches,"activation_max_abs":maximum,"redzones_unchanged":true,"fixture_bytes":bytes,"passed":passed])
                }
                try require(passing.contains(where:{$0.name==variants[0].name}),"weighted baseline changed on repeat")
                for round in 0..<6 {
                    let ordered=round%2==0 ? passing : passing.reversed().map{$0}
                    for v in ordered {
                        let time=try runner.ffn(v,weights,x,candidate,n:n)
                        let flops=6.0*Double(n)*4096*12288
                        try emit(["kind":"timing","name":v.name,"n":n,"round":round,"gpu_ms":time.gpu*1000,"wall_ms":time.wall*1000,"projection_tflops":flops/time.gpu/1e12,"load_average":loadAverage(),"projection_flops":flops,"scope":"three GEMMs and SiLU; no residual, Q4 conversion, head, or serving overhead"])
                    }
                }
            }
        }
    }
    try emit(["kind":"complete","small_oracle_passing_variants":valid.sorted(),"gpu_executed":true,"model_executed":false,"all_variants_small_passed":valid.count==variants.count])
} catch {
    try? emit(["kind":"failure","error":String(describing:error)])
    exit(1)
}
