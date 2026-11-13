// Firm reason: 6 (Metal SDK effects for the Coax-authored offscreen verification plan).
// No realm/shader arithmetic or pass planning lives here.
import Foundation
import Metal

enum SDKFailure: Error {case failed(String)}
func need<T>(_ value:T?,_ why:String)throws->T {guard let value else{throw SDKFailure.failed(why)};return value}
func number(_ value:Any?)->Int {(value as? NSNumber)?.intValue ?? 0}
func flag(_ value:Any?)->Bool {(value as? NSNumber)?.boolValue ?? false}
func word(_ value:Any?)->UInt32 {(value as? NSNumber)?.uint32Value ?? 0}
func integers(_ value:Any?)->[Int] {(value as? [NSNumber] ?? []).map(\.intValue)}
func dataOfWords(_ words:[UInt32])->Data {var d=Data();for var w in words.map({$0.littleEndian}){withUnsafeBytes(of:&w){d.append(contentsOf:$0)}};return d}
func run()throws {
    guard CommandLine.arguments.count==4 else{throw SDKFailure.failed("usage: metal-sdk PLAN.json OUT.f32 REPORT.json")}
    let planData=try Data(contentsOf:URL(fileURLWithPath:CommandLine.arguments[1]))
    let plan=try need(JSONSerialization.jsonObject(with:planData) as? [String:Any],"Coax plan is not an object")
    guard plan["schema"] as? String == "coax.metal-g5.v1" else{throw SDKFailure.failed("unsupported Coax plan")}
    let device=try need(MTLCreateSystemDefaultDevice(),"no Metal device"),queue=try need(device.makeCommandQueue(),"no Metal command queue")
    let resources=try need(plan["resources"] as? [[String:Any]],"no resources"),modules=try need(plan["modules"] as? [[String:Any]],"no modules")
    var buffers=[MTLBuffer?](repeating:nil,count:resources.count),textures=[MTLTexture?](repeating:nil,count:resources.count),samplers=[MTLSamplerState?](repeating:nil,count:resources.count)
    for (i,r) in resources.enumerated() {
        let file=r["file"] as? String ?? ""
        switch number(r["kind"]) {
        case 1:
            var d = file.isEmpty ? dataOfWords((r["bits"] as? [NSNumber] ?? []).map(\.uint32Value)) : try Data(contentsOf:URL(fileURLWithPath:file))
            guard d.count==number(r["bytes"]) else{throw SDKFailure.failed("buffer bytes changed after Coax admission")}
            if d.count<16{d.append(Data(repeating:0,count:16-d.count))}
            buffers[i]=try d.withUnsafeBytes{try need(device.makeBuffer(bytes:$0.baseAddress!,length:d.count,options:.storageModeShared),"buffer allocation")}
        case 2:
            let f=number(r["format"]),desc=MTLTextureDescriptor()
            desc.textureType=f==3 ? .type2DArray:.type2D
            desc.pixelFormat=f==1 ? .rgba16Float:f==2 ? .depth32Float:f==3 ? .rgba8Unorm_srgb:.rgba8Unorm
            desc.width=number(r["width"]);desc.height=number(r["height"]);desc.arrayLength=number(r["layers"]);desc.mipmapLevelCount=number(r["levels"])
            desc.usage = f<=2 ? [.shaderRead,.renderTarget]:[.shaderRead]
            desc.storageMode=f==2 ? .private:.shared
            let texture=try need(device.makeTexture(descriptor:desc),"texture allocation: \(r["name"] ?? i)");textures[i]=texture
            if !file.isEmpty {
                var data=try Data(contentsOf:URL(fileURLWithPath:file))
                guard data.count==number(r["bytes"]) else{throw SDKFailure.failed("texture bytes changed after Coax admission")}
                if flag(r["atlas"]){var rgba=Data(capacity:data.count*4);for a in data{rgba.append(contentsOf:[a,a,a,a])};data=rgba}
                try data.withUnsafeBytes { raw in
                    var at=0
                    for level in 0..<desc.mipmapLevelCount {
                        let w=max(1,desc.width>>level),h=max(1,desc.height>>level),bytes=w*h*4
                        for slice in 0..<desc.arrayLength {
                            guard at+bytes<=data.count else{throw SDKFailure.failed("short mip upload")}
                            texture.replace(region:MTLRegionMake2D(0,0,w,h),mipmapLevel:level,slice:slice,withBytes:raw.baseAddress!.advanced(by:at),bytesPerRow:w*4,bytesPerImage:bytes);at+=bytes
                        }
                    }
                    guard at==data.count else{throw SDKFailure.failed("trailing mip bytes")}
                }
            }
        case 3:
            let d=MTLSamplerDescriptor();d.minFilter=flag(r["linear"]) ? .linear:.nearest;d.magFilter=d.minFilter
            d.mipFilter=flag(r["mip"]) ? .linear:.nearest
            d.sAddressMode=flag(r["repeat"]) ? .repeat:.clampToEdge;d.tAddressMode=d.sAddressMode;d.rAddressMode=d.sAddressMode
            d.compareFunction=flag(r["compare"]) ? .less:.never;d.lodMaxClamp=Float.greatestFiniteMagnitude
            samplers[i]=try need(device.makeSamplerState(descriptor:d),"sampler allocation")
        default:throw SDKFailure.failed("unknown admitted resource effect")
        }
    }
    let indexPath=plan["index"] as? String ?? ""
    var indices:MTLBuffer?
    if !indexPath.isEmpty {let d=try Data(contentsOf:URL(fileURLWithPath:indexPath));indices=try d.withUnsafeBytes{try need(device.makeBuffer(bytes:$0.baseAddress!,length:d.count,options:.storageModeShared),"index upload")}}
    let options=MTLCompileOptions();options.languageVersion = .version3_2;options.mathMode = .safe;options.mathFloatingPointFunctions = .precise
    var libraries:[MTLLibrary]=[]
    for m in modules {libraries.append(try device.makeLibrary(source:String(contentsOfFile:try need(m["file"] as? String,"shader source"),encoding:.utf8),options:options))}
    let commands=try need(plan["commands"] as? [[String:Any]],"no GPU effects"),cb=try need(queue.makeCommandBuffer(),"command buffer")
    var pipelines:[String:MTLRenderPipelineState]=[:],depthStates:[String:MTLDepthStencilState]=[:],passes=0,draws=0
    let factors:[MTLBlendFactor]=[.zero,.one,.sourceAlpha,.oneMinusSourceAlpha,.sourceColor]
    for c in commands {
        if c["op"] as? String == "copy" {
            let a=try need(textures[number(c["from"])],"copy source"),b=try need(textures[number(c["to"])],"copy destination"),e=try need(cb.makeBlitCommandEncoder(),"blit encoder")
            e.copy(from:a,sourceSlice:0,sourceLevel:0,sourceOrigin:MTLOrigin(x:0,y:0,z:0),sourceSize:MTLSize(width:a.width,height:a.height,depth:1),to:b,destinationSlice:0,destinationLevel:0,destinationOrigin:MTLOrigin(x:0,y:0,z:0));e.endEncoding();continue
        }
        guard c["op"] as? String == "pass" else{throw SDKFailure.failed("unknown admitted command effect")}
        let mi=number(c["module"]),m=modules[mi],colors=integers(c["colors"]),depth=number(c["depth"]),blend=integers(c["blend"])
        let key="\(mi):\(colors.count):\(depth>=0):\(blend)"
        if pipelines[key]==nil {
            let p=MTLRenderPipelineDescriptor();p.vertexFunction=try need(libraries[mi].makeFunction(name:try need(m["vertex"] as? String,"vertex entry")),"vertex function")
            p.fragmentFunction=try need(libraries[mi].makeFunction(name:try need(m["fragment"] as? String,"fragment entry")),"fragment function")
            for (i,id) in colors.enumerated() {
                let a=p.colorAttachments[i]!,off=blend[6]&(1<<i) != 0
                a.pixelFormat=try need(textures[id],"color target").pixelFormat;a.writeMask=off ? []:.all;a.isBlendingEnabled=blend[0] != 0 && !off
                a.rgbBlendOperation=blend[0]==7 ? .max:.add;a.sourceRGBBlendFactor=factors[blend[1]];a.destinationRGBBlendFactor=factors[blend[2]]
                a.alphaBlendOperation=blend[3]==7 ? .max:.add;a.sourceAlphaBlendFactor=factors[blend[4]];a.destinationAlphaBlendFactor=factors[blend[5]]
            }
            if depth>=0 {p.depthAttachmentPixelFormat=try need(textures[depth],"depth target").pixelFormat}
            let attrs=m["attributes"] as? [[NSNumber]] ?? []
            if !attrs.isEmpty {
                let vd=MTLVertexDescriptor(),formats:[[MTLVertexFormat]]=[[],[],[.float,.float2,.float3,.float4],[.int,.int2,.int3,.int4],[.uint,.uint2,.uint3,.uint4]]
                for raw in attrs {let a=raw.map(\.intValue),field=vd.attributes[a[0]]!;field.format=formats[a[1]][a[2]-1];field.offset=a[3];field.bufferIndex=30;vd.layouts[30].stride=a[4];vd.layouts[30].stepFunction = .perVertex}
                p.vertexDescriptor=vd
            }
            pipelines[key]=try device.makeRenderPipelineState(descriptor:p)
        }
        let rp=MTLRenderPassDescriptor(),keep=flag(c["keep"])
        for (i,id) in colors.enumerated(){let a=rp.colorAttachments[i]!;a.texture=textures[id];a.loadAction=keep ? .load:.clear;a.storeAction = .store;a.clearColor=MTLClearColorMake(0,0,0,0)}
        if depth>=0{rp.depthAttachment.texture=textures[depth];rp.depthAttachment.loadAction=keep ? .load:.clear;rp.depthAttachment.storeAction = .store;rp.depthAttachment.clearDepth=1}
        let e=try need(cb.makeRenderCommandEncoder(descriptor:rp),"render encoder");e.setRenderPipelineState(pipelines[key]!);e.setCullMode(.none);e.setFrontFacing(.counterClockwise)
        if depth>=0 {
            let dk="\(flag(c["depthWrite"])):\(flag(c["depthAlways"]))"
            if depthStates[dk]==nil{let d=MTLDepthStencilDescriptor();d.isDepthWriteEnabled=flag(c["depthWrite"]);d.depthCompareFunction=flag(c["depthAlways"]) ? .always:.less;depthStates[dk]=try need(device.makeDepthStencilState(descriptor:d),"depth state")}
            e.setDepthStencilState(depthStates[dk]);e.setDepthBias(Float(bitPattern:word(c["biasBits"])),slopeScale:Float(bitPattern:word(c["slopeBits"])),clamp:0)
        }
        for raw in c["bindings"] as? [[NSNumber]] ?? [] {let b=raw.map(\.intValue),slot=b[1],id=b[2]
            switch b[0] {case 1:e.setVertexBuffer(buffers[id],offset:0,index:slot);e.setFragmentBuffer(buffers[id],offset:0,index:slot)
            case 2:e.setVertexTexture(textures[id],index:slot);e.setFragmentTexture(textures[id],index:slot)
            case 3:e.setVertexSamplerState(samplers[id],index:slot);e.setFragmentSamplerState(samplers[id],index:slot)
            default:throw SDKFailure.failed("unknown binding effect")}
        }
        let lengths=dataOfWords((c["lengths"] as? [NSNumber] ?? []).map(\.uint32Value));lengths.withUnsafeBytes{e.setVertexBytes($0.baseAddress!,length:$0.count,index:29);e.setFragmentBytes($0.baseAddress!,length:$0.count,index:29)}
        let vb=number(c["vertexBuffer"]);if vb>=0{e.setVertexBuffer(buffers[vb],offset:0,index:30)}
        for d in c["draws"] as? [[Any]] ?? [] {
            if flag(d[0]) {e.drawIndexedPrimitives(type:.triangle,indexCount:number(d[2]),indexType:.uint32,indexBuffer:try need(indices,"indexed draw without indices"),indexBufferOffset:number(d[1])*4,instanceCount:number(d[4]),baseVertex:0,baseInstance:number(d[3]))}
            else{e.drawPrimitives(type:.triangle,vertexStart:number(d[1]),vertexCount:number(d[2]),instanceCount:number(d[4]),baseInstance:number(d[3]))};draws+=1
        }
        e.endEncoding();passes+=1
    }
    cb.commit();cb.waitUntilCompleted();if let error=cb.error{throw error}
    var output=Data()
    for id in integers(plan["outputs"]) {
        let t=try need(textures[id],"output texture");var raw=[UInt16](repeating:0,count:t.width*t.height*4)
        raw.withUnsafeMutableBytes{t.getBytes($0.baseAddress!,bytesPerRow:t.width*8,from:MTLRegionMake2D(0,0,t.width,t.height),mipmapLevel:0)}
        for h in raw {var bits=Float(Float16(bitPattern:UInt16(littleEndian:h))).bitPattern.littleEndian;withUnsafeBytes(of:&bits){output.append(contentsOf:$0)}}
    }
    try output.write(to:URL(fileURLWithPath:CommandLine.arguments[2]),options:.atomic)
    let result:[String:Any]=["schema":"coax.metal-g5-result.v1","device":device.name,"passes":passes,"draws":draws,"libraries":libraries.count,"outputBytes":output.count,"gpuSeconds":cb.gpuEndTime-cb.gpuStartTime,"completed":cb.status == .completed]
    try JSONSerialization.data(withJSONObject:result,options:[.sortedKeys]).write(to:URL(fileURLWithPath:CommandLine.arguments[3]),options:.atomic)
    print("Metal SDK: \(passes) passes, \(draws) draws, \(output.count) output bytes on \(device.name)")
}
do{try run()}catch{fputs("Metal SDK: \(error)\n",stderr);exit(1)}
