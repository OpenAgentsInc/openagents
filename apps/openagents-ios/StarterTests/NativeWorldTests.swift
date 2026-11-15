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
            c.reading=(NativeWorldHost.text(profile.reading) as NSString).deletingPathExtension;c.title=(NativeWorldHost.text(profile.heading) as NSString).deletingPathExtension;c.titleWeight=600;c.titleScale=Double(profile.title_scale)
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
                    // character, with the same front door as a player.
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
    static func microduckConfig(_ pack:URL)->AEPageConfig {
        let repo=URL(fileURLWithPath:#filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let c=AEPageConfig(),size=CGSize(width:430,height:900)
        c.producer=Bundle.main.privateFrameworksURL!.appendingPathComponent("MicroduckRuntime.framework/MicroduckRuntime").path
        c.shaders=Bundle.main.resourceURL!.appendingPathComponent("WorldShaders/microduck").path;c.pack=pack.path
        c.fonts=repo.appendingPathComponent("apps/openagents-web/realms/splice-bots/assets/fonts").path;c.reading="IBMPlexMono-Medium";c.title="Michroma-Regular"
        c.store=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).path;c.path="/microduck";c.origin="https://openagents.com";c.realm="microduck";c.slot="microduck";c.offline=true;c.automated=true;c.touch=true;c.screen=size
        return c
    }
    func testMicroduckRendersActualGeometryOrbitPinchAndRefusesAnAlteredPack()throws {
        let device=try XCTUnwrap(MTLCreateSystemDefaultDevice()),queue=try XCTUnwrap(device.makeCommandQueue())
        let repo=URL(fileURLWithPath:#filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let size=CGSize(width:430,height:900),pack=repo.appendingPathComponent("apps/openagents-ios/build/world-assets/microduck.pack")
        let c=Self.microduckConfig(pack)
        var page:AEPageHost?,why:NSString?
        NativeWorldHost.deep.run {page=AEPageHost.open(c,device:device,queue:queue,why:&why)}
        let viewer=try XCTUnwrap(page,"Microduck: \(why ?? "unknown")");defer {NativeWorldHost.deep.run {viewer.close()}}
        func frame(_ name:String,dt:Float=0.016)throws->Data {
            var pixels=Data(),image:UIImage?
            NativeWorldHost.deep.run {
                guard let texture=viewer.frame(size,scale:1,dt:dt,wait:true) else {return}
                let row=((texture.width*4+255)/256)*256,total=row*texture.height
                guard let read=device.makeBuffer(length:total,options:.storageModeShared),let cmd=queue.makeCommandBuffer(),let copy=cmd.makeBlitCommandEncoder() else {return}
                copy.copy(from:texture,sourceSlice:0,sourceLevel:0,sourceOrigin:MTLOrigin(x:0,y:0,z:0),sourceSize:MTLSize(width:texture.width,height:texture.height,depth:1),to:read,destinationOffset:0,destinationBytesPerRow:row,destinationBytesPerImage:total);copy.endEncoding();cmd.commit();cmd.waitUntilCompleted()
                pixels=Data(bytes:read.contents(),count:total)
                if let provider=CGDataProvider(data:pixels as CFData),let cg=CGImage(width:texture.width,height:texture.height,bitsPerComponent:8,bitsPerPixel:32,bytesPerRow:row,space:CGColorSpaceCreateDeviceRGB(),bitmapInfo:CGBitmapInfo(rawValue:CGImageAlphaInfo.premultipliedFirst.rawValue).union(.byteOrder32Little),provider:provider,decode:nil,shouldInterpolate:false,intent:.defaultIntent) {image=UIImage(cgImage:cg)}
            }
            XCTAssertFalse(pixels.isEmpty);let picture=try XCTUnwrap(image);let shot=XCTAttachment(image:picture);shot.name=name;shot.lifetime = .keepAlways;add(shot);return pixels
        }
        func changed(_ a:Data,_ b:Data)->Int {
            guard a.count==b.count else {return 0};let row=((430*4+255)/256)*256
            return a.withUnsafeBytes {aa in b.withUnsafeBytes {bb in let x=aa.bindMemory(to:UInt8.self),y=bb.bindMemory(to:UInt8.self);var n=0
                // Excludes HUD captions: only actual assembly pixels can change.
                for v in 250..<680 {for u in 80..<350 {let i=v*row+u*4;if abs(Int(x[i])-Int(y[i]))+abs(Int(x[i+1])-Int(y[i+1]))+abs(Int(x[i+2])-Int(y[i+2]))>15 {n+=1}}};return n
            }}
        }
        let material=try frame("Microduck native materials")
        func items()throws->[[String:Any]] {
            var raw:Data?;NativeWorldHost.deep.run {raw=viewer.viewMenu()}
            let recipe=try JSONSerialization.jsonObject(with:XCTUnwrap(raw)) as! [String:Any]
            XCTAssertEqual(recipe["schema"] as? String,"metaverse.engine.view-menu.v1")
            return try XCTUnwrap(recipe["items"] as? [[String:Any]])
        }
        XCTAssertEqual(try items().filter {$0["group"] as? String=="Joints"}.count,14)
        XCTAssertEqual(try items().filter {$0["group"] as? String=="Parts"}.count,70)
        NativeWorldHost.deep.run {_ = viewer.key(1007,mods:0,down:true);for _ in 0..<4 {_ = viewer.key(43,mods:0,down:true)}}
        let head=try frame("Microduck native articulated head");XCTAssertGreaterThan(changed(material,head),200)
        NativeWorldHost.deep.run {_ = viewer.key(120,mods:0,down:true)}
        let exploded=try frame("Microduck native exploded assembly");XCTAssertGreaterThan(changed(head,exploded),1000)
        NativeWorldHost.deep.run {_ = viewer.key(114,mods:0,down:true)}
        let reset=try frame("Microduck native restored stand");XCTAssertEqual(changed(material,reset),0)
        NativeWorldHost.deep.run {viewer.touch(CGPoint(x:220,y:350),phase:0,finger:9);viewer.touch(CGPoint(x:220,y:350),phase:2,finger:9)}
        let tapped=try items().filter {$0["group"] as? String=="Parts" && $0["current"] as? Bool==true}
        XCTAssertEqual(tapped.count,1,"A geometry tap must select one named source part")
        let selected=try frame("Microduck native tapped part"),pickedKey=try XCTUnwrap(tapped.first?["key"] as? Int32)
        NativeWorldHost.deep.run {_ = viewer.key(pickedKey,mods:0,down:true)}
        XCTAssertEqual(changed(selected,try frame("Microduck native named part")),0)
        NativeWorldHost.deep.run {_ = viewer.key(114,mods:0,down:true)}
        NativeWorldHost.deep.run {_ = viewer.key(110,mods:0,down:true)}
        let normals=try frame("Microduck native normals");XCTAssertGreaterThan(changed(material,normals),1000,"The surface pass did not draw the model")
        NativeWorldHost.deep.run {_ = viewer.key(110,mods:0,down:true);viewer.touch(CGPoint(x:230,y:450),phase:0,finger:8);viewer.touch(CGPoint(x:310,y:460),phase:1,finger:8);viewer.touch(CGPoint(x:310,y:460),phase:2,finger:8)}
        let orbit=try frame("Microduck native orbit");XCTAssertGreaterThan(changed(material,orbit),1000)
        NativeWorldHost.deep.run {viewer.pinch(1.4)}
        let zoom=try frame("Microduck native pinch");XCTAssertGreaterThan(changed(orbit,zoom),1000)
        // Independent byte observations of the actual compiled producer. The SDK
        // only copies the registered SLV1 packet; numerics stay in Coax/Psionic.
        func lab()throws->Data {var data:Data?;NativeWorldHost.deep.run {data=viewer.servoLab()};return try XCTUnwrap(data)}
        func word(_ data:Data,_ offset:Int)->UInt32 {data.withUnsafeBytes {$0.loadUnaligned(fromByteOffset:offset,as:UInt32.self).littleEndian}}
        func number(_ data:Data,_ offset:Int)->Double {Double(bitPattern:data.withUnsafeBytes {$0.loadUnaligned(fromByteOffset:offset,as:UInt64.self).littleEndian})}
        NativeWorldHost.deep.run {_ = viewer.key(114,mods:0,down:true);_ = viewer.key(1007,mods:0,down:true);_ = viewer.key(3000,mods:0,down:true)}
        _ = try frame("Microduck servo initial",dt:0);_ = try frame("Microduck servo independent first step",dt:0.002)
        let first=try lab();XCTAssertEqual(first.count,312);XCTAssertEqual(word(first,224),1)
        XCTAssertEqual(number(first,272),0.2,accuracy:1e-14);XCTAssertEqual(number(first,280),0.15)
        NativeWorldHost.deep.run {_ = viewer.key(3001,mods:0,down:true)}
        _ = try frame("Microduck delayed initial",dt:0);_ = try frame("Microduck delayed first step",dt:0.002)
        let delayed=try lab();XCTAssertEqual(word(delayed,12),16);XCTAssertEqual(number(delayed,240),0);XCTAssertEqual(number(delayed,248),0)
        for _ in 0..<30 {NativeWorldHost.deep.run {_ = viewer.frame(size,scale:1,dt:0.016,wait:true)}}
        _ = try frame("Microduck native delayed response")
        let savedRun=try XCTUnwrap(viewer.servoRun())
        NativeWorldHost.deep.run {_ = viewer.key(3200,mods:0,down:true)}
        _ = try frame("Microduck native saved experiment",dt:0)
        NativeWorldHost.deep.run {_ = viewer.key(3000,mods:0,down:true);_ = viewer.key(3201,mods:0,down:true)}
        _ = try frame("Microduck native saved replay timeline",dt:0)
        XCTAssertEqual(viewer.servoRun(),savedRun)
        NativeWorldHost.deep.run {_ = viewer.key(3002,mods:0,down:true)}
        _ = try frame("Microduck limited initial",dt:0);_ = try frame("Microduck native limited effort",dt:0.002)
        let limited=try lab();XCTAssertEqual(number(limited,272),0.2,accuracy:1e-14);XCTAssertEqual(number(limited,280),0.03)
        NativeWorldHost.deep.run {_ = viewer.key(3003,mods:0,down:true)}
        let held=try lab();NativeWorldHost.deep.run {_ = viewer.key(3011,mods:0,down:true)}
        let refusedPacket=try lab();XCTAssertEqual(word(refusedPacket,216),3)
        for i in 0..<312 where !(216..<220).contains(i) {XCTAssertEqual(held[i],refusedPacket[i],"Refusal changed accepted byte \(i)")}
        _ = try frame("Microduck native refused inertia")
        let bad=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString+".pack");try FileManager.default.copyItem(at:pack,to:bad);defer {try? FileManager.default.removeItem(at:bad)}
        let file=try FileHandle(forUpdating:bad);try file.seek(toOffset:10000);let old=try XCTUnwrap(file.read(upToCount:1));try file.seek(toOffset:10000);try file.write(contentsOf:Data([old[0]^1]));try file.close()
        c.pack=bad.path;var refused:AEPageHost?;why=nil;NativeWorldHost.deep.run {refused=AEPageHost.open(c,device:device,queue:queue,why:&why)}
        XCTAssertNil(refused);XCTAssertTrue((why as String? ?? "").contains("SHA256"),"\(why ?? "missing refusal")")
    }

}
