import Foundation
import Metal
import CryptoKit
import Darwin

struct Failure: Error, CustomStringConvertible {let description:String;init(_ s:String){description=s}}
func require(_ value:Bool,_ message:String)throws{if !value{throw Failure(message)}}
func emit(_ row:[String:Any])throws{FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject:row,options:[.sortedKeys]));FileHandle.standardOutput.write(Data([10]))}
func sha(_ pointer:UnsafeRawPointer,_ size:Int)->String{SHA256.hash(data:Data(bytesNoCopy:UnsafeMutableRawPointer(mutating:pointer),count:size,deallocator:.none)).map{String(format:"%02x",$0)}.joined()}
func loadAverage()->[Double]{var a=[Double](repeating:0,count:3);_ = getloadavg(&a,3);return a}
let root=URL(fileURLWithPath:CommandLine.arguments[1]),mode=CommandLine.arguments[2]
try require(["check","bench"].contains(mode),"mode must be check or bench")
let redzone=256,poison:UInt8=0xa5,maxFixtureBytes=1536*1024*1024
let variants=["mpp","mpp_shapes","expanded_tile_f16","q4_tile_f16","native_uint4_affine"]
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
        let source=try String(contentsOf:root.appendingPathComponent("baseline.metal"),encoding:.utf8)+String(contentsOf:root.appendingPathComponent("candidates.metal"),encoding:.utf8)
        let options=MTLCompileOptions()
        let library=try d.makeLibrary(source:source,options:options)
        for name in ["clef_gemm","clef_gemm_accumulate","clef_gemm_64x128","clef_gemm_64x128_accumulate","probe_silu","probe_add_residual","verify_decode","q4_tile_f16","expanded_tile_f16","activation_sums","native_uint4_affine","native_uint4_raw"]{
            guard let function=library.makeFunction(name:name)else{throw Failure("missing kernel \(name)")}
            let pipeline=try d.makeComputePipelineState(function:function)
            try require(pipeline.staticThreadgroupMemoryLength<=d.maxThreadgroupMemoryLength,"threadgroup memory exceeds device limit")
            let helper=["probe_silu","probe_add_residual","verify_decode"].contains(name)
            let threads=name=="activation_sums" ? 32:(helper ? 256:128)
            try require(pipeline.maxTotalThreadsPerThreadgroup>=threads,"pipeline cannot support its required thread count")
            if !helper{try require(pipeline.threadExecutionWidth==32,"tensor and reduction kernels require 32-wide SIMD groups")}
            pipelines[name]=pipeline
        }
        try emit(["kind":"device","name":d.name,"mode":mode,"conditions":"shared-machine scratch Q4 projection diagnostic","max_threadgroup_bytes":d.maxThreadgroupMemoryLength,"decoded_tile_static_bytes":pipelines["q4_tile_f16"]!.staticThreadgroupMemoryLength])
    }
    func encode(_ command:MTLCommandBuffer,_ name:String,_ buffers:[Buffer],_ dims:[UInt32],_ groups:MTLSize,_ threads:Int){
        let e=command.makeComputeCommandEncoder()!;e.setComputePipelineState(pipelines[name]!);for(i,b)in buffers.enumerated(){e.setBuffer(b.raw,offset:redzone,index:i)}
        dims.withUnsafeBytes{e.setBytes($0.baseAddress!,length:$0.count,index:buffers.count)}
        e.dispatchThreadgroups(groups,threadsPerThreadgroup:MTLSize(width:threads,height:1,depth:1));e.endEncoding()
    }
    func submit(_ encode:(MTLCommandBuffer)->Void)throws->Timing{
        try autoreleasepool{let began=ProcessInfo.processInfo.systemUptime;let command=queue.makeCommandBuffer()!;encode(command);command.commit();command.waitUntilCompleted();try require(command.status == .completed,"command failed: \(String(describing:command.error))");let gpu=command.gpuEndTime-command.gpuStartTime;try require(gpu.isFinite&&gpu>0,"invalid timestamp");return Timing(gpu:gpu,wall:ProcessInfo.processInfo.systemUptime-began)}
    }
}
final class Weight{
    let m:Int,k:Int,q4:Buffer,half:Buffer,uint4:Buffer,scales:Buffer,offsets:Buffer
    var buffers:[Buffer]{[q4,half,uint4,scales,offsets]}
    init(_ r:Runner,m:Int,k:Int)throws{try require(k%256==0,"K must contain whole Q4_K blocks");self.m=m;self.k=k
        q4=try Buffer(r.device,m*k/256*144,1);half=try Buffer(r.device,m*k,2);uint4=try Buffer(r.device,m*k/2,1);scales=try Buffer(r.device,m*k/32,4);offsets=try Buffer(r.device,m*k/32,4)
    }
    func synthesize(_ raw:Bool){
        let b=q4.pointer.bindMemory(to:UInt8.self,capacity:q4.count),h=half.pointer.bindMemory(to:Float16.self,capacity:half.count),u=uint4.pointer.bindMemory(to:UInt8.self,capacity:uint4.count),a=scales.pointer.bindMemory(to:Float.self,capacity:scales.count),o=offsets.pointer.bindMemory(to:Float.self,capacity:offsets.count)
        for block in 0..<m*k/256{
            let at=block*144,d=Float16(raw ? 1:Float(block%7+1)/256),dm=Float16(raw ? 0:Float(block%5+1)/1024)
            b[at]=UInt8(d.bitPattern&255);b[at+1]=UInt8(d.bitPattern>>8);b[at+2]=UInt8(dm.bitPattern&255);b[at+3]=UInt8(dm.bitPattern>>8)
            for i in 0..<12{b[at+4+i]=raw ? (i<4 ? 1:(i<8 ? 0:1)):UInt8((block*19+i*23)%256)}
            for i in 0..<128{b[at+16+i]=UInt8((block*17+i*31)%256)}
            for e in 0..<256{
                let g=e/32,sc=g<4 ? b[at+4+g]&63:(b[at+4+g+4]&15)|((b[at+4+g-4]>>6)<<4),mn=g<4 ? b[at+4+g+4]&63:(b[at+4+g+4]>>4)|((b[at+4+g]>>6)<<4)
                let packed=b[at+16+(e/64)*32+e%32],q=e%64>=32 ? packed>>4:packed&15,index=block*256+e
                let scale=Float(d)*Float(sc),offset = -(Float(dm)*Float(mn));a[block*8+g]=scale;o[block*8+g]=offset
                h[index]=Float16(offset.addingProduct(scale,Float(q)))
                if index%2==0{u[index/2]=q}else{u[index/2]|=q<<4}
            }
        }
    }
    func load(_ root:URL,_ name:String)throws{
        let base=root.appendingPathComponent("artifacts"),manifest=try JSONSerialization.jsonObject(with:Data(contentsOf:base.appendingPathComponent("manifest.json"))) as! [String:Any]
        let entry=(manifest["tensors"] as! [[String:Any]]).first{$0["name"] as? String==name}!,files=entry["files"] as! [String:[String:Any]]
        try require(entry["m"] as! Int==m && entry["k"] as! Int==k,"weight shape mismatch")
        for(key,buffer)in zip(["q4","f16","uint4","scales.f32","offsets.f32"],buffers){try autoreleasepool{
            let row=files[key]!,data=try Data(contentsOf:base.appendingPathComponent(row["file"] as! String),options:.mappedIfSafe)
            try require(data.count==buffer.count*buffer.bytes && SHA256.hash(data:data).map{String(format:"%02x",$0)}.joined()==row["sha256"] as! String,"weight identity failed")
            data.withUnsafeBytes{buffer.pointer.copyMemory(from:$0.baseAddress!,byteCount:data.count)}
        }}
    }
}
final class Projection{
    let runner:Runner,n:Int,w:Weight,x:Buffer,output:Buffer,sums:Buffer,residual:Buffer,isDown:Bool
    init(_ r:Runner,n:Int,w:Weight,x:Buffer,isDown:Bool)throws{runner=r;self.n=n;self.w=w;self.x=x;self.isDown=isDown
        output=try Buffer(r.device,n*w.m,4);sums=try Buffer(r.device,n*w.k/32,4);residual=try Buffer(r.device,n*w.m,4)
        let a=residual.pointer.bindMemory(to:Float.self,capacity:residual.count);for i in 0..<residual.count{a[i]=Float(i%101-50)/100}
    }
    func poisonOutput(){output.pointer.bindMemory(to:Float.self,capacity:output.count).initialize(repeating:Float.nan,count:output.count);sums.pointer.bindMemory(to:Float.self,capacity:sums.count).initialize(repeating:Float.nan,count:sums.count)}
    func run(_ variant:String)throws->Timing{
        try runner.submit{c in
            let dims:[UInt32]=[UInt32(n),UInt32(w.m),UInt32(w.k),0],wide=variant=="mpp_shapes" && ((w.m==12288&&w.k==4096)||(w.m==4096&&w.k==12288)),tile=wide ? 128:64,groups=MTLSize(width:(w.m+tile-1)/tile,height:(n+63)/64,depth:1)
            if isDown && variant.hasPrefix("mpp") {
                let b=c.makeBlitCommandEncoder()!;b.copy(from:residual.raw,sourceOffset:redzone,to:output.raw,destinationOffset:redzone,size:n*w.m*4);b.endEncoding()
            }
            if variant=="native_uint4_affine"{
                runner.encode(c,"activation_sums",[x,sums],[UInt32(n),UInt32(w.k)],MTLSize(width:n*w.k/32,height:1,depth:1),32)
                runner.encode(c,variant,[x,w.uint4,w.scales,w.offsets,sums,output],dims,groups,128)
            }else if variant=="native_uint4_raw"{runner.encode(c,variant,[x,w.uint4,output],dims,groups,128)
            }else if variant.hasPrefix("mpp"){runner.encode(c,(wide ? "clef_gemm_64x128":"clef_gemm")+(isDown ? "_accumulate":""),[x,w.half,output],dims,groups,128)
            }else{runner.encode(c,variant,[x,w.q4,w.half,output],dims,groups,128)}
            // Candidates add the residual in f32 after their K loop. The MPP
            // reference retains the exact current accumulate-kernel behavior.
            if isDown && !variant.hasPrefix("mpp"){runner.encode(c,"probe_add_residual",[output,residual],[UInt32(n*w.m)],MTLSize(width:(n*w.m+255)/256,height:1,depth:1),256)}
        }
    }
    func guards()throws{for b in w.buffers+[x,output,sums,residual]{try b.guards()}}
}
func oracle(_ p:Projection)->[Double]{
    let x=p.x.pointer.bindMemory(to:Float16.self,capacity:p.x.count),w=p.w.half.pointer.bindMemory(to:Float16.self,capacity:p.w.half.count),r=p.residual.pointer.bindMemory(to:Float.self,capacity:p.residual.count)
    var out=[Double](repeating:0,count:p.n*p.w.m)
    for n in 0..<p.n{for m in 0..<p.w.m{var a=0.0;for k in 0..<p.w.k{a+=Double(x[n*p.w.k+k])*Double(w[m*p.w.k+k])};out[n*p.w.m+m]=a+(p.isDown ? Double(r[n*p.w.m+m]):0)}};return out
}
var numericalPass=true
func small(_ r:Runner)throws{
    for(raw,n,m,k)in [(true,1,1,256),(true,17,65,256),(false,3,7,256),(false,17,65,512)]{for down in [false,true]{try autoreleasepool{
        let w=try Weight(r,m:m,k:k);w.synthesize(raw);let x=try Buffer(r.device,n*k,2);fill(x,k,19,0.125)
        let p=try Projection(r,n:n,w:w,x:x,isDown:down),expected=oracle(p),sources=w.buffers+[x,p.residual],hashes=sources.map{$0.hash()}
        for v in variants+(raw ? ["native_uint4_raw"]:[]){p.poisonOutput();_=try p.run(v);try p.guards()
            let a=p.output.pointer.bindMemory(to:Float.self,capacity:p.output.count);var error=0.0,scaled=0.0
            for i in 0..<p.output.count{let v=Double(a[i]);try require(v.isFinite,"small output nonfinite");let d=abs(v-expected[i]);error=max(error,d);scaled=max(scaled,d/max(1,abs(expected[i])))}
            let pass = v=="native_uint4_affine" && !raw ? error<=1e-3:scaled<=3e-5
            numericalPass = numericalPass && pass
            try emit(["kind":"small_oracle","variant":v,"raw_uint4_fixture":raw,"n":n,"m":m,"k":k,"residual":down,"max_abs":error,"max_scaled_abs":scaled,"passed":pass,"gate":v=="native_uint4_affine" && !raw ? "absolute1e-3":"scaled3e-5"])
        }
        try require(hashes==sources.map{$0.hash()},"source mutation")
    }}}
}
func verifyWeight(_ r:Runner,_ w:Weight)throws{
    let decoded=try Buffer(r.device,w.m*w.k,2)
    _=try r.submit{r.encode($0,"verify_decode",[w.q4,decoded],[UInt32(w.m*w.k)],MTLSize(width:(w.m*w.k+255)/256,height:1,depth:1),256)}
    try decoded.guards();try require(decoded.hash()==w.half.hash(),"Q4_K decoded half bits differ from independent decoder")
    try emit(["kind":"decoded_half_identity","values":w.m*w.k,"all_bits_equal":true])
}
do{
    let r=try Runner();try small(r)
    if mode=="bench"{for n in [1024,2048]{try autoreleasepool{
        let x=try Buffer(r.device,n*4096,2),gate=try Buffer(r.device,n*12288,4),up=try Buffer(r.device,n*12288,4),activation=try Buffer(r.device,n*12288,2);fill(x,4096,19,1)
        for name in ["gate","up","down"]{try autoreleasepool{
            let down=name=="down",m=down ? 4096:12288,k=down ? 12288:4096,w=try Weight(r,m:m,k:k);try w.load(root,name);try verifyWeight(r,w)
            if down{_=try r.submit{r.encode($0,"probe_silu",[gate,up,activation],[UInt32(n*12288)],MTLSize(width:(n*12288+255)/256,height:1,depth:1),256)}}
            let p=try Projection(r,n:n,w:w,x:down ? activation:x,isDown:down),reference=try Buffer(r.device,n*m,4)
            let bytes=(w.buffers+[x,gate,up,activation,p.output,p.sums,p.residual,reference]).reduce(0){$0+$1.raw.length}
            try require(bytes<=maxFixtureBytes,"fixture exceeds1.5GiB")
            let source=w.buffers+[p.x,p.residual],hashes=source.map{$0.hash()}
            p.poisonOutput();_=try p.run("mpp");reference.pointer.copyMemory(from:p.output.pointer,byteCount:n*m*4)
            if !down{(name=="gate" ? gate:up).pointer.copyMemory(from:p.output.pointer,byteCount:n*m*4)}
            for v in variants{p.poisonOutput();_=try p.run(v);try p.guards();var row=try statistics(p.output,reference);let pass=(row["max_abs"] as! Double)<=1e-3;numericalPass=numericalPass&&pass
                row.merge(["kind":"weighted_correctness","variant":v,"projection":name,"n":n,"m":m,"k":k,"fixture_bytes":bytes,"strict_absolute_1e_3_passed":pass,"full_model_gate_applied":false],uniquingKeysWith:{$1});try emit(row)
            }
            for round in 0..<4{for v in round%2==0 ? variants:Array(variants.reversed()){let t=try p.run(v);try emit(["kind":"timing","variant":v,"projection":name,"n":n,"round":round,"gpu_ms":t.gpu*1000,"wall_ms":t.wall*1000,"load_average":loadAverage(),"scope":"single projection; affine input sums and separate f32 residual included; no full-model claim"])} }
            try p.guards();try require(hashes==source.map{$0.hash()},"weighted source mutation")
        }}
    }}}
    try emit(["kind":"complete","numerical_pass":numericalPass,"full_model_gate_applied":false]);exit(numericalPass ? 0:2)
}catch{try? emit(["kind":"failure","error":"\(error)"]);exit(1)}
