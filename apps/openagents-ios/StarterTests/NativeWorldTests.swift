// Firm reason: 6 (independent XCTest/Metal SDK oracle of the shipped native producers).
import XCTest
import MetalKit
@testable import OpenAgents

@MainActor final class NativeWorldTests:XCTestCase {
    func testInstalledWorldsRenderAndTakeInputAbove32BitAddresses()throws {
        let device=try XCTUnwrap(MTLCreateSystemDefaultDevice()),queue=try XCTUnwrap(device.makeCommandQueue())
        let repo=URL(fileURLWithPath:#filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        for realm in ["splice-bots","everglade","everglade2","myrealm"] {
            let profile=realm.withCString {nwr_profile(coax_string(data:$0,count:Int64(realm.utf8.count)))}
            let c=AEPageConfig(),framework=NativeWorldHost.text(profile.framework)
            c.producer=Bundle.main.privateFrameworksURL!.appendingPathComponent("\(framework).framework/\(framework)").path
            c.shaders=Bundle.main.resourceURL!.appendingPathComponent("WorldShaders/\(realm)").path
            let web=repo.appendingPathComponent("apps/openagents-web")
            c.pack=NativeWorldHost.text(profile.pack).isEmpty ? "":web.appendingPathComponent(NativeWorldHost.text(profile.pack)).path
            c.fonts=web.appendingPathComponent(NativeWorldHost.text(profile.font_dir)).path
            c.reading=(NativeWorldHost.text(profile.reading) as NSString).deletingPathExtension;c.title=(NativeWorldHost.text(profile.heading) as NSString).deletingPathExtension;c.titleWeight=600
            c.path=realm=="myrealm" ? "/worlds/test-native":"/worlds/\(realm)";c.origin="https://openagents.com";c.web=web.path
            c.realm=realm;c.slot=realm;c.style=NativeWorldHost.text(profile.style);c.clock=NativeWorldHost.text(profile.clock);c.variant=NativeWorldHost.text(profile.variant)
            c.store=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).path;c.offline=true;c.automated=true;c.touch=true;c.screen=CGSize(width:780,height:420)
            var page:AEPageHost?,why:NSString?,image:UIImage?
            NativeWorldHost.deep.run {
                page=AEPageHost.open(c,device:device,queue:queue,why:&why)
                guard let page else {return}
                for _ in 0..<3 {_ = page.frame(CGSize(width:780,height:420),scale:1,dt:0.016,wait:true)}
                if realm=="splice-bots" {
                    // Walk the shipped front door: New game, then Use this
                    // character. The historical stage 1 is an obsolete spawn.
                    for _ in 0..<2 {_ = page.key(-2,mods:0,down:true);_ = page.key(-2,mods:0,down:false);_ = page.frame(CGSize(width:780,height:420),scale:1,dt:0.016,wait:true)}
                }
                page.touch(CGPoint(x:120,y:280),phase:0,finger:11);page.touch(CGPoint(x:120,y:200),phase:1,finger:11)
                for _ in 0..<5 {_ = page.frame(CGSize(width:780,height:420),scale:1,dt:0.016,wait:true)}
                page.touch(CGPoint(x:120,y:200),phase:3,finger:11)
                if let texture=page.frame(CGSize(width:780,height:420),scale:1,dt:0.016,wait:true) {
                    let row=((texture.width*4+255)/256)*256,bytes=row*texture.height
                    let read=device.makeBuffer(length:bytes,options:.storageModeShared)!,cmd=queue.makeCommandBuffer()!,blit=cmd.makeBlitCommandEncoder()!
                    blit.copy(from:texture,sourceSlice:0,sourceLevel:0,sourceOrigin:MTLOrigin(x:0,y:0,z:0),sourceSize:MTLSize(width:texture.width,height:texture.height,depth:1),to:read,destinationOffset:0,destinationBytesPerRow:row,destinationBytesPerImage:bytes);blit.endEncoding();cmd.commit();cmd.waitUntilCompleted()
                    let data=Array(UnsafeBufferPointer(start:read.contents().assumingMemoryBound(to:UInt8.self),count:bytes))
                    let colours=Set(stride(from:0,to:bytes,by:4).map {UInt32(data[$0]) | UInt32(data[$0+1])<<8 | UInt32(data[$0+2])<<16})
                    XCTAssertGreaterThan(colours.count,32,"\(realm) produced a blank frame")
                    let provider=CGDataProvider(data:Data(data) as CFData)!
                    if let cg=CGImage(width:texture.width,height:texture.height,bitsPerComponent:8,bitsPerPixel:32,bytesPerRow:row,space:CGColorSpaceCreateDeviceRGB(),bitmapInfo:CGBitmapInfo(rawValue:CGImageAlphaInfo.premultipliedFirst.rawValue).union(.byteOrder32Little),provider:provider,decode:nil,shouldInterpolate:false,intent:.defaultIntent) {image=UIImage(cgImage:cg)}
                }
                page.close()
            }
            XCTAssertNotNil(page,"\(realm): \(why ?? "unknown")")
            if let image {let shot=XCTAttachment(image:image);shot.name="native-\(realm)";shot.lifetime = .keepAlways;add(shot)}
        }
    }
}
