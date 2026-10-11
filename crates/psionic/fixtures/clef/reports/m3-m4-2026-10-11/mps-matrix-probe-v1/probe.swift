import Foundation
import Metal
import MetalPerformanceShaders
import CryptoKit
import Darwin

struct Failure: Error, CustomStringConvertible {let description:String;init(_ s:String){description=s}}
func require(_ value:Bool,_ message:String)throws{if !value{throw Failure(message)}}
func emit(_ row:[String:Any])throws{FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject:row,options:[.sortedKeys]));FileHandle.standardOutput.write(Data([10]))}
func sha(_ pointer:UnsafeRawPointer,_ size:Int)->String{SHA256.hash(data:Data(bytesNoCopy:UnsafeMutableRawPointer(mutating:pointer),count:size,deallocator:.none)).map{String(format:"%02x",$0)}.joined()}
func loadAverage()->[Double]{var a=[Double](repeating:0,count:3);_ = getloadavg(&a,3);return a}
let root=URL(fileURLWithPath:CommandLine.arguments[1]),mode=CommandLine.arguments[2]
try require(["mixed-check","check","bench","bench-mixed"].contains(mode),"mode must be mixed-check, check, bench, or bench-mixed")
let redzone=256,poison:UInt8=0xa5,maxFixtureBytes=1536*1024*1024
let variants=mode=="mixed-check" ? ["mps_mixed"] : ["mpp","mpp_shapes","mps_f32","mps_f16"]+(mode=="bench-mixed" ? ["mps_mixed"]:[])
final class Buffer {
    let raw:MTLBuffer,count:Int,bytes:Int
    init(_ device:MTLDevice,_ count:Int,_ bytes:Int)throws{
        self.count=count;self.bytes=bytes
        try require(count>0 && count*bytes+2*redzone<=maxFixtureBytes,"buffer size exceeds bound")
        guard let b=device.makeBuffer(length:count*bytes+2*redzone,options:.storageModeShared)else{throw Failure("allocation failed")};raw=b
        raw.contents().initializeMemory(as:UInt8.self,repeating:poison,count:raw.length)
    }
    var pointer:UnsafeMutableRawPointer{raw.contents().advanced(by:redzone)}
    func guards()throws{let p=raw.contents().bindMemory(to:UInt8.self,capacity:raw.length);for i in 0..<redzone{try require(p[i]==poison && p[redzone+count*bytes+i]==poison,"redzone changed")}}
    func hash()->String{sha(pointer,count*bytes)}
}
func fill(_ buffer:Buffer,_ width:Int,_ seed:Int,_ scale:Float){let p=buffer.pointer.bindMemory(to:Float16.self,capacity:buffer.count);for i in 0..<buffer.count{p[i]=Float16(Float((i*31+(i/width)*17+seed)%251-125)/125*scale)}}
func widen(_ input:Buffer,_ output:Buffer){let a=input.pointer.bindMemory(to:Float16.self,capacity:input.count),b=output.pointer.bindMemory(to:Float.self,capacity:output.count);for i in 0..<input.count{b[i]=Float(a[i])}}
func statistics(_ actual:Buffer,_ reference:Buffer)throws->[String:Any]{
    let a=actual.pointer.bindMemory(to:Float.self,capacity:actual.count),b=reference.pointer.bindMemory(to:Float.self,capacity:reference.count)
    var maximum=0.0,scaled=0.0,squares=0.0,referenceSquares=0.0,differences=0,over=0
    for i in 0..<actual.count{let x=Double(a[i]),y=Double(b[i]);try require(x.isFinite && y.isFinite,"nonfinite output");let d=abs(x-y);maximum=max(maximum,d);scaled=max(scaled,d/max(1,abs(y)));squares+=d*d;referenceSquares+=y*y;if a[i].bitPattern != b[i].bitPattern{differences+=1};if d>1e-3{over+=1}}
    return ["values":actual.count,"max_abs":maximum,"max_scaled_abs":scaled,"rms":sqrt(squares/Double(actual.count)),"relative_rms":sqrt(squares/max(referenceSquares,Double.leastNormalMagnitude)),"bit_differences":differences,"values_over_1e_3":over,"finite":true]
}
struct Timing{let gpu:Double,wall:Double}
final class Runner{
    let device:MTLDevice,queue:MTLCommandQueue
    var pipelines=[String:MTLComputePipelineState]()
    init()throws{
        guard let d=MTLCreateSystemDefaultDevice(),let q=d.makeCommandQueue()else{throw Failure("Metal unavailable")};device=d;queue=q
        let library=try d.makeLibrary(source:String(contentsOf:root.appendingPathComponent("kernels.metal"),encoding:.utf8),options:MTLCompileOptions())
        for name in ["clef_gemm","clef_gemm_accumulate","clef_gemm_64x128","clef_gemm_64x128_accumulate","probe_widen","probe_half_result","probe_silu"]{pipelines[name]=try d.makeComputePipelineState(function:library.makeFunction(name:name)!)}
        try emit(["kind":"device","name":d.name,"recommended_working_set":d.recommendedMaxWorkingSetSize,"mode":mode,"conditions":"shared machine; bounded kernel diagnostic"])
    }
    func compute(_ command:MTLCommandBuffer,_ name:String,_ buffers:[Buffer],_ dimensions:[UInt32],_ count:Int){
        let e=command.makeComputeCommandEncoder()!;e.setComputePipelineState(pipelines[name]!)
        for (index,buffer) in buffers.enumerated(){e.setBuffer(buffer.raw,offset:redzone,index:index)}
        dimensions.withUnsafeBytes{e.setBytes($0.baseAddress!,length:$0.count,index:buffers.count)}
        e.dispatchThreadgroups(MTLSize(width:(count+255)/256,height:1,depth:1),threadsPerThreadgroup:MTLSize(width:256,height:1,depth:1));e.endEncoding()
    }
    func submit(_ encode:(MTLCommandBuffer)->Void)throws->Timing{
        try autoreleasepool{let start=ProcessInfo.processInfo.systemUptime;let command=queue.makeCommandBuffer()!;encode(command);command.commit();command.waitUntilCompleted();try require(command.status == .completed,"command failed: \(String(describing:command.error))");let gpu=command.gpuEndTime-command.gpuStartTime;try require(gpu.isFinite && gpu>0,"invalid GPU timestamp");return Timing(gpu:gpu,wall:ProcessInfo.processInfo.systemUptime-start)}
    }
}
final class Projection {
    let runner:Runner,n:Int,m:Int,k:Int,residual:Bool
    let x16:Buffer,w16:Buffer,x32:Buffer,w32:Buffer,output:Buffer,halfOutput:Buffer,addend:Buffer
    var kernels=[String:MPSMatrixMultiplication](),matrices=[String:[MPSMatrix]]()
    init(_ runner:Runner,n:Int,m:Int,k:Int,x:Buffer,w:Buffer,residual:Bool)throws{
        self.runner=runner;self.n=n;self.m=m;self.k=k;self.residual=residual;x16=x;w16=w
        let d=runner.device
        x32=try Buffer(d,n*k,4);w32=try Buffer(d,m*k,4);output=try Buffer(d,n*m,4);halfOutput=try Buffer(d,n*m,2);addend=try Buffer(d,n*m,4)
        widen(w16,w32)
        let p=addend.pointer.bindMemory(to:Float.self,capacity:n*m);for i in 0..<n*m{p[i]=Float(i%101-50)/100}
        func matrix(_ buffer:Buffer,_ rows:Int,_ columns:Int,_ type:MPSDataType)->MPSMatrix{MPSMatrix(buffer:buffer.raw,offset:redzone,descriptor:MPSMatrixDescriptor(rows:rows,columns:columns,rowBytes:columns*buffer.bytes,dataType:type))}
        for variant in variants where !variant.hasPrefix("mpp") {
            let fp32=variant=="mps_f32",halfResult=variant=="mps_f16"
            kernels[variant]=MPSMatrixMultiplication(device:d,transposeLeft:false,transposeRight:true,resultRows:n,resultColumns:m,interiorColumns:k,alpha:1,beta:residual && !halfResult ? 1:0)
            matrices[variant]=[matrix(fp32 ? x32:x16,n,k,fp32 ? .float32:.float16),matrix(fp32 ? w32:w16,m,k,fp32 ? .float32:.float16),matrix(halfResult ? halfOutput:output,n,m,halfResult ? .float16:.float32)]
        }
    }
    func resetResult(){
        output.pointer.bindMemory(to:Float.self,capacity:n*m).initialize(repeating:Float(bitPattern:0x7fc00555),count:n*m)
        halfOutput.pointer.bindMemory(to:Float16.self,capacity:n*m).initialize(repeating:Float16.nan,count:n*m)
    }
    func run(_ variant:String)throws->Timing{
        try runner.submit{command in
            // Restore the same f32 residual for every sample; this copy is timed.
            if residual{let blit=command.makeBlitCommandEncoder()!;blit.copy(from:addend.raw,sourceOffset:redzone,to:output.raw,destinationOffset:redzone,size:n*m*4);blit.endEncoding()}
            if variant.hasPrefix("mpp") {
                // Match the production weight-shape selector for these Flash FFN shapes.
                let wide=variant=="mpp_shapes" && ((m==12288 && k==4096) || (m==4096 && k==12288))
                let tile=wide ? 128:64,base=wide ? "clef_gemm_64x128":"clef_gemm"
                let e=command.makeComputeCommandEncoder()!;e.setComputePipelineState(runner.pipelines[base+(residual ? "_accumulate":"")]!)
                e.setBuffer(x16.raw,offset:redzone,index:0);e.setBuffer(w16.raw,offset:redzone,index:1);e.setBuffer(output.raw,offset:redzone,index:2)
                var dims=SIMD4<UInt32>(UInt32(n),UInt32(m),UInt32(k),0);e.setBytes(&dims,length:16,index:3)
                e.dispatchThreadgroups(MTLSize(width:(m+tile-1)/tile,height:(n+63)/64,depth:1),threadsPerThreadgroup:MTLSize(width:128,height:1,depth:1));e.endEncoding()
            } else {
                if variant=="mps_f32"{runner.compute(command,"probe_widen",[x16,x32],[UInt32(n*k)],n*k)}
                let matrices=matrices[variant]!;kernels[variant]!.encode(commandBuffer:command,leftMatrix:matrices[0],rightMatrix:matrices[1],resultMatrix:matrices[2])
                if variant=="mps_f16"{runner.compute(command,"probe_half_result",[halfOutput,addend,output],[UInt32(n*m),residual ? 1:0],n*m)}
            }
        }
    }
    func guards()throws{for buffer in [x16,w16,x32,w32,output,halfOutput,addend]{try buffer.guards()}}
}
func oracle(_ p:Projection)throws->[Double]{
    let x=p.x16.pointer.bindMemory(to:Float16.self,capacity:p.n*p.k),w=p.w16.pointer.bindMemory(to:Float16.self,capacity:p.m*p.k),r=p.addend.pointer.bindMemory(to:Float.self,capacity:p.n*p.m)
    var values=[Double](repeating:0,count:p.n*p.m)
    for row in 0..<p.n{for col in 0..<p.m{var value=0.0;for t in 0..<p.k{value+=Double(x[row*p.k+t])*Double(w[col*p.k+t])};values[row*p.m+col]=value+(p.residual ? Double(r[row*p.m+col]):0)}}
    return values
}
func small(_ runner:Runner)throws{
    for (n,m,k) in [(1,1,17),(3,7,65),(7,65,129),(65,17,32)]{for residual in [false,true]{try autoreleasepool{
        let x=try Buffer(runner.device,n*k,2),w=try Buffer(runner.device,m*k,2)
        fill(x,k,19,1);fill(w,k,41,1/sqrt(Float(k)))
        let p=try Projection(runner,n:n,m:m,k:k,x:x,w:w,residual:residual),expected=try oracle(p)
        let before=[x.hash(),w.hash(),p.w32.hash()]
        for variant in variants{
            try emit(["kind":"attempt","variant":variant,"n":n,"m":m,"k":k,"residual":residual,"mixed_support_unproven":variant=="mps_mixed"])
            p.resetResult();_=try p.run(variant);try p.guards()
            let values=p.output.pointer.bindMemory(to:Float.self,capacity:n*m)
            var maximum=0.0,scaled=0.0
            for i in 0..<n*m{let actual=Double(values[i]);try require(actual.isFinite,"small output nonfinite");let error=abs(actual-expected[i]);maximum=max(maximum,error);scaled=max(scaled,error/max(1,abs(expected[i])))}
            let bound=variant=="mps_f16" ? 1e-3:3e-5
            try emit(["kind":"small_oracle","variant":variant,"n":n,"m":m,"k":k,"residual":residual,"max_abs":maximum,"max_scaled_abs":scaled,"diagnostic_limit":bound,"passed":scaled<=bound,"full_model_gate_applied":false])
            try require(scaled<=bound,"small independent oracle failed")
        }
        try require(before==[x.hash(),w.hash(),p.w32.hash()],"input or weight mutation")
    }}}
}
func weight(_ runner:Runner,_ name:String)throws->Buffer{
    try autoreleasepool{
        let manifest=try JSONSerialization.jsonObject(with:Data(contentsOf:root.appendingPathComponent("weights.json"))) as! [String:Any]
        let entry=(manifest["tensors"] as! [[String:Any]]).first{$0["half_file"] as? String==name+".f16"}!
        let data=try Data(contentsOf:root.appendingPathComponent(name+".f16"),options:.mappedIfSafe)
        try require(data.count==4096*12288*2 && SHA256.hash(data:data).map{String(format:"%02x",$0)}.joined()==entry["half_sha256"] as! String,"weight identity failed")
        let buffer=try Buffer(runner.device,data.count/2,2);data.withUnsafeBytes{buffer.pointer.copyMemory(from:$0.baseAddress!,byteCount:data.count)}
        try emit(["kind":"weight","name":name,"bytes":data.count,"sha256":entry["half_sha256"]!]);return buffer
    }
}
do{
    let runner=try Runner();try small(runner)
    if mode.hasPrefix("bench"){
        for n in [1024,2048]{try autoreleasepool{
            let x=try Buffer(runner.device,n*4096,2),gate=try Buffer(runner.device,n*12288,4),up=try Buffer(runner.device,n*12288,4),activation=try Buffer(runner.device,n*12288,2)
            fill(x,4096,19,1)
            for name in ["gate","up","down"]{try autoreleasepool{
                let down=name=="down",m=down ? 4096:12288,k=down ? 12288:4096,input=down ? activation:x
                if down{_=try runner.submit{runner.compute($0,"probe_silu",[gate,up,activation],[UInt32(n*12288)],n*12288)}}
                let w=try weight(runner,name),p=try Projection(runner,n:n,m:m,k:k,x:input,w:w,residual:down)
                let fixtureBytes=[x,gate,up,activation,w,p.x32,p.w32,p.output,p.halfOutput,p.addend].reduce(0){$0+$1.raw.length}
                try require(fixtureBytes+n*m*4+2*redzone<=maxFixtureBytes,"total fixture exceeds predeclared bound")
                let before=[input.hash(),w.hash(),p.w32.hash()]
                p.resetResult();_=try p.run("mpp")
                let reference=try Buffer(runner.device,n*m,4);reference.pointer.copyMemory(from:p.output.pointer,byteCount:n*m*4)
                if !down{(name=="gate" ? gate:up).pointer.copyMemory(from:p.output.pointer,byteCount:n*m*4)}
                for variant in variants{
                    p.resetResult();_=try p.run(variant);try p.guards();var row=try statistics(p.output,reference)
                    row.merge(["kind":"weighted_correctness","variant":variant,"projection":name,"n":n,"m":m,"k":k,"residual":down,"fixture_bytes":fixtureBytes+reference.raw.length,"precision":"\(variant)","full_model_gate_applied":false,"strict_full_model_gate_still_required":true],uniquingKeysWith:{$1});try emit(row)
                }
                for round in 0..<4{let order=round%2==0 ? variants:Array(variants.reversed());for variant in order{let t=try p.run(variant);try emit(["kind":"timing","variant":variant,"projection":name,"n":n,"round":round,"gpu_ms":t.gpu*1000,"wall_ms":t.wall*1000,"projection_tflops":2.0*Double(n)*Double(m)*Double(k)/t.gpu/1e12,"load_average":loadAverage(),"scope":"one projection; input widening and residual copy/post-add included; staged weight widening excluded"])} }
                try p.guards();try require(before==[input.hash(),w.hash(),p.w32.hash()],"weighted input or weight mutation")
            }}
        }}
    }
    try emit(["kind":"complete","mode":mode,"gpu_executed":true,"model_executed":false,"full_model_gate_applied":false])
}catch{try? emit(["kind":"failure","error":"\(error)"]);exit(1)}
