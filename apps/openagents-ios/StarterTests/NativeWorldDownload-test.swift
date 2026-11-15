// Firm reason: 6 (independent URLSession/filesystem oracle of signed inventory delivery).
import XCTest
import CryptoKit
import MetalKit
@testable import OpenAgents

private final class WorldDownloadOracle:URLProtocol {
    static let lock=NSLock()
    static var body=Data(),status=200,requests:[URLRequest]=[]
    static func reset(_ data:Data,status:Int=200) {lock.lock();body=data;self.status=status;requests=[];lock.unlock()}
    static var count:Int {lock.lock();defer {lock.unlock()};return requests.count}
    override class func canInit(with request:URLRequest)->Bool {true}
    override class func canonicalRequest(for request:URLRequest)->URLRequest {request}
    override func startLoading() {
        Self.lock.lock();Self.requests.append(request);let body=Self.body,status=Self.status;Self.lock.unlock()
        let r=HTTPURLResponse(url:request.url!,statusCode:status,httpVersion:"HTTP/1.1",headerFields:["Content-Length":"\(body.count)"])!
        client?.urlProtocol(self,didReceive:r,cacheStoragePolicy:.notAllowed);client?.urlProtocol(self,didLoad:body);client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}
@MainActor final class NativeWorldDownloadTests:XCTestCase {
    func testDefaultWorldPathsAreInsideTheWritableIOSLibrary()async throws {
        let support=FileManager.default.urls(for:.applicationSupportDirectory,in:.userDomainMask)[0].standardizedFileURL
        let assets=NativeWorldAssets(),home=await assets.home
        XCTAssertEqual(home,NativeWorldStorage.directory("worlds"))
        XCTAssertTrue(home.path.hasPrefix(support.path+"/"))
        for leaf in ["worlds/players/account.json","world-state/splice-bots"] {
            XCTAssertTrue(NativeWorldStorage.directory(leaf).path.hasPrefix(support.path+"/"))
        }
    }
    func testDownloadsWorkWhenTheContainerRootRefusesNewDirectories()async throws {
        let fm=FileManager.default,container=fm.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let support=container.appendingPathComponent("Library/Application Support")
        try fm.createDirectory(at:support,withIntermediateDirectories:true)
        try fm.setAttributes([.posixPermissions:0o500],ofItemAtPath:container.path)
        defer {try? fm.setAttributes([.posixPermissions:0o700],ofItemAtPath:container.path);try? fm.removeItem(at:container)}
        XCTAssertThrowsError(try fm.createDirectory(at:container.appendingPathComponent(".openagents/worlds/objects"),withIntermediateDirectories:true))
        let data=Data("world in an iOS-writable directory".utf8),inventory=try self.inventory(data)
        let root=NativeWorldStorage.directory("worlds",base:support)
        WorldDownloadOracle.reset(data);let assets=cache(root)
        let installed=try await assets.prepare(inventory,progress:{_,_ in})
        XCTAssertEqual(try Data(contentsOf:installed.appendingPathComponent("base.pack")),data)
        WorldDownloadOracle.reset(Data(),status:503)
        _ = try await assets.prepare(inventory,progress:{_,_ in});XCTAssertEqual(WorldDownloadOracle.count,0)
        // Player credentials and game saves use the same writable logical home.
        for leaf in ["worlds/players/account.json","world-state/splice-bots/save"] {
            let file=NativeWorldStorage.directory(leaf,base:support)
            try fm.createDirectory(at:file.deletingLastPathComponent(),withIntermediateDirectories:true)
            try data.write(to:file,options:.atomic)
            XCTAssertEqual(try Data(contentsOf:file),data)
        }
    }
    func testMicroduckSignedInventoryStreamsAndReopensOffline()async throws {
        let bundle=try XCTUnwrap(Bundle.main.url(forResource:"Starter",withExtension:nil))
        let inventory=try NativeWorldInventory(Data(contentsOf:bundle.appendingPathComponent("worlds/microduck.json")))
        let versions=try PropertyListSerialization.propertyList(from:Data(contentsOf:XCTUnwrap(Bundle.main.url(forResource:"WorldRuntimes",withExtension:"plist"))),format:nil) as! [String:String]
        XCTAssertEqual(inventory.realm,"microduck");XCTAssertEqual(inventory.runtime,versions["microduck"])
        let root=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString);defer {try? FileManager.default.removeItem(at:root)}
        let assets=NativeWorldAssets(home:root),installed=try await assets.prepare(inventory,progress:{_,_ in})
        let result=try NativeWorldAssets.hash(installed.appendingPathComponent("base.pack"))
        XCTAssertEqual(result.0,50965060);XCTAssertEqual(result.1,"fc21cb95595d665bb83dc5b90c373dcf197f5b45b34b376c0d110b044bb408b5")
        WorldDownloadOracle.reset(Data(),status:503);let offline=cache(root)
        let reopened=try await offline.prepare(inventory,progress:{_,_ in});XCTAssertEqual(reopened,installed);XCTAssertEqual(WorldDownloadOracle.count,0)
        // Reopen the actually downloaded cache in the actual Metal producer,
        // with all page network effects disabled, then save/reopen its run.
        let device=try XCTUnwrap(MTLCreateSystemDefaultDevice()),queue=try XCTUnwrap(device.makeCommandQueue())
        let c=NativeWorldTests.microduckConfig(reopened.appendingPathComponent("base.pack")),size=CGSize(width:430,height:900)
        var page:AEPageHost?,why:NSString?,saved:Data?,opened:Data?,branch:Data?,numerical:Data?
        NativeWorldHost.deep.run {
            page=AEPageHost.open(c,device:device,queue:queue,why:&why)
            guard let page else {return}
            _ = page.key(1007,mods:0,down:true);_ = page.key(3001,mods:0,down:true)
            _ = page.frame(size,scale:1,dt:0,wait:true)
            for _ in 0..<10 {_ = page.frame(size,scale:1,dt:0.002,wait:true)}
            saved=page.servoRun()
            _ = page.key(3200,mods:0,down:true)
            _ = page.frame(size,scale:1,dt:0,wait:true)
            _ = page.key(3000,mods:0,down:true);_ = page.key(3201,mods:0,down:true)
            _ = page.frame(size,scale:1,dt:0,wait:true);opened=page.servoRun()
            _ = page.key(3205,mods:0,down:true);_ = page.key(3204,mods:0,down:true)
            _ = page.frame(size,scale:1,dt:0,wait:true)
            _ = page.frame(size,scale:1,dt:0.004,wait:true)
            _ = page.key(3208,mods:0,down:true);branch=page.servoRun()
            _ = page.frame(size,scale:1,dt:0,wait:true)
            for _ in 0..<100 {_ = page.frame(size,scale:1,dt:0.002,wait:true)}
            _ = page.key(3004,mods:0,down:true)
            for _ in 0..<50 {_ = page.frame(size,scale:1,dt:0.002,wait:true)}
            numerical=page.servoRun()
            page.close()
        }
        XCTAssertNotNil(page,"\(why ?? "Open failed")")
        let artifact=try XCTUnwrap(saved),replay=try XCTUnwrap(opened),fork=try XCTUnwrap(branch)
        XCTAssertEqual(artifact.count,1088);XCTAssertEqual(replay,artifact)
        XCTAssertEqual(fork.subdata(in:48..<80),artifact.suffix(32));XCTAssertNotEqual(fork,artifact)
        XCTAssertEqual(WorldDownloadOracle.count,0,"Cached producer and local replay made no download request")
        let shot=XCTAttachment(data:artifact,uniformTypeIdentifier:"public.data");shot.name="Microduck native saved run SLR1";shot.lifetime = .keepAlways;add(shot)
        let values=try XCTUnwrap(numerical);XCTAssertEqual(values.count,1100)
        let position=Double(bitPattern:values.withUnsafeBytes {$0.loadUnaligned(fromByteOffset:896,as:UInt64.self).littleEndian})
        XCTAssertNotEqual(position,0)
        let sample=XCTAttachment(data:values,uniformTypeIdentifier:"public.data");sample.name="Microduck native numerical replay SLR1";sample.lifetime = .keepAlways;add(sample)
    }
    private func inventory(_ data:Data)throws->NativeWorldInventory {
        let hash=NativePackage.hex(Data(SHA256.hash(data:data)))
        return try NativeWorldInventory(JSONSerialization.data(withJSONObject:["schema":"coax.native.world.assets.v1","realm":"splice-bots","runtime":String(repeating:"a",count:64),"files":[["path":"base.pack","sha256":hash,"bytes":data.count,"required":true]]]))
    }
    private func cache(_ root:URL)->NativeWorldAssets {
        let c=URLSessionConfiguration.ephemeral;c.protocolClasses=[WorldDownloadOracle.self]
        return NativeWorldAssets(home:root,transport:NativeWorldSession(configuration:c))
    }
    func testWholeHashCacheRevalidationAndOfflineReopen()async throws {
        let data=Data("verified world bytes".utf8),q=try inventory(data),root=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer {try? FileManager.default.removeItem(at:root)}
        WorldDownloadOracle.reset(data);let assets=cache(root)
        let installed=try await assets.prepare(q,progress:{_,_ in})
        XCTAssertEqual(try Data(contentsOf:installed.appendingPathComponent("base.pack")),data);XCTAssertEqual(WorldDownloadOracle.count,1)
        WorldDownloadOracle.reset(Data(),status:503)
        _ = try await assets.prepare(q,progress:{_,_ in});XCTAssertEqual(WorldDownloadOracle.count,0,"offline cache is sufficient")
        try Data("corrupt".utf8).write(to:installed.appendingPathComponent("base.pack"))
        WorldDownloadOracle.reset(data);_ = try await assets.prepare(q,progress:{_,_ in});XCTAssertEqual(WorldDownloadOracle.count,1)
        XCTAssertEqual(try Data(contentsOf:installed.appendingPathComponent("base.pack")),data)
    }
    func testBadHashTruncationAndOversizeNeverPublishAFile()async throws {
        for received in [Data("abd".utf8),Data("ab".utf8),Data("abcd".utf8)] {
            let q=try inventory(Data("abc".utf8)),root=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString);defer {try? FileManager.default.removeItem(at:root)}
            let assets=cache(root);WorldDownloadOracle.reset(received)
            do {_ = try await assets.prepare(q,progress:{_,_ in});XCTFail("unverified bytes were accepted")}catch {}
            let installed=await assets.directory(q);XCTAssertFalse(FileManager.default.fileExists(atPath:installed.appendingPathComponent("base.pack").path))
            let files=(try? FileManager.default.contentsOfDirectory(atPath:root.appendingPathComponent("objects").path)) ?? []
            XCTAssertFalse(files.contains { $0.hasSuffix(".partial") })
        }
    }
    func testCancelledAdmissionCannotCreateAnInstalledWorld()async throws {
        let q=try inventory(Data("abc".utf8)),root=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString);defer {try? FileManager.default.removeItem(at:root)}
        let assets=cache(root);WorldDownloadOracle.reset(Data("abc".utf8))
        let task=Task {_ = try await assets.prepare(q,progress:{_,_ in})};task.cancel()
        do {try await task.value;XCTFail("cancelled download accepted")}catch {}
        let installed=await assets.directory(q);XCTAssertFalse(FileManager.default.fileExists(atPath:installed.appendingPathComponent("base.pack").path))
    }
}
