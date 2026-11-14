import XCTest
import CryptoKit
@testable import OpenAgents

final class NativePackageTests:XCTestCase {
    let runtime=String(repeating:"a",count:64)
    func envelope(_ key:Curve25519.Signing.PrivateKey,path:String="index.html",generation:Int=1,bytes:Int?=nil,runtime:String?=nil,content:Data=Data("abc".utf8)) throws -> Data {
        let object:[String:Any]=["schema":"coax.native.package.v1","app":"com.openagents.app","runtime":runtime ?? self.runtime,"generation":generation,"files":[["path":path,"bytes":bytes ?? content.count,"sha256":NativePackage.hex(Data(SHA256.hash(data:content)))]]]
        let raw=try JSONSerialization.data(withJSONObject:object,options:.sortedKeys)
        var n=UInt32(raw.count).littleEndian; var result=Data("CNU1".utf8)
        withUnsafeBytes(of:&n) { result.append(contentsOf:$0) }; result.append(raw)
        result.append(try key.signature(for:Data("Coax Native Package v1\0".utf8)+raw)); return result
    }
    func testSignedPackageAndIndependentArtifact() throws {
        let key=Curve25519.Signing.PrivateKey(), dir=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at:dir,withIntermediateDirectories:true); defer { try? FileManager.default.removeItem(at:dir) }
        let p=try NativePackage(envelope:envelope(key),app:"com.openagents.app",runtime:runtime,publicKey:key.publicKey.rawRepresentation)
        XCTAssertEqual(p.generation,1); XCTAssertEqual(p.files.count,1)
        try Data("abc".utf8).write(to:dir.appendingPathComponent("index.html")); try p.validate(dir)
        try Data("abd".utf8).write(to:dir.appendingPathComponent("index.html")); XCTAssertThrowsError(try p.validate(dir))
    }
    func testSignatureRuntimeAppGenerationAndTruncationRefuse() throws {
        let key=Curve25519.Signing.PrivateKey(); let good=try envelope(key)
        var bad=good; bad[bad.count-1] ^= 1
        for data in [bad,Data(good.dropLast()),Data()] { XCTAssertThrowsError(try NativePackage(envelope:data,app:"com.openagents.app",runtime:runtime,publicKey:key.publicKey.rawRepresentation)) }
        XCTAssertThrowsError(try NativePackage(envelope:good,app:"another.app",runtime:runtime,publicKey:key.publicKey.rawRepresentation))
        XCTAssertThrowsError(try NativePackage(envelope:good,app:"com.openagents.app",runtime:String(repeating:"b",count:64),publicKey:key.publicKey.rawRepresentation))
        XCTAssertThrowsError(try NativePackage(envelope:good,app:"com.openagents.app",runtime:runtime,publicKey:key.publicKey.rawRepresentation,floor:1))
        for path in ["../escape","/index.html","a//index.html","index.html?x"] {
            XCTAssertThrowsError(try NativePackage(envelope:envelope(key,path:path),app:"com.openagents.app",runtime:runtime,publicKey:key.publicKey.rawRepresentation))
        }
    }
    @MainActor func testFreshInstallRecognizesBundledContentsAndStillOffersChangedCode() throws {
        let key=Curve25519.Signing.PrivateKey(),home=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer {try? FileManager.default.removeItem(at:home)}
        let updates=try NativeUpdates(home:home,verificationKey:key.publicKey.rawRepresentation)
        XCTAssertEqual(updates.boot(),updates.embedded)
        let content=try Data(contentsOf:updates.embedded.appendingPathComponent("index.html"))
        let same=try NativePackage(envelope:envelope(key,generation:9,runtime:updates.runtime,content:content),app:updates.app,runtime:updates.runtime,publicKey:updates.publicKey)
        XCTAssertFalse(try updates.requiresDownload(same))
        XCTAssertEqual(updates.read("highwater.cnu")?.generation,9)
        XCTAssertNil(updates.pending)
        XCTAssertEqual(try FileManager.default.contentsOfDirectory(atPath:home.path),["highwater.cnu"])
        // Higher generation alone is not new code. An actual change is still offered.
        let changed=try NativePackage(envelope:envelope(key,generation:10,runtime:updates.runtime),app:updates.app,runtime:updates.runtime,publicKey:updates.publicKey)
        XCTAssertTrue(try updates.requiresDownload(changed))
        XCTAssertEqual(updates.read("highwater.cnu")?.generation,9)
        let old=try NativePackage(envelope:envelope(key,generation:8,runtime:updates.runtime),app:updates.app,runtime:updates.runtime,publicKey:updates.publicKey)
        XCTAssertFalse(try updates.requiresDownload(old))
        let restarted=try NativeUpdates(home:home,verificationKey:key.publicKey.rawRepresentation)
        XCTAssertFalse(try restarted.requiresDownload(same))
    }
    @MainActor func testRunningUpdateDeduplicatesOnlyCompleteMatchingFiles() throws {
        let key=Curve25519.Signing.PrivateKey(),home=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer {try? FileManager.default.removeItem(at:home)}
        let updates=try NativeUpdates(home:home,verificationKey:key.publicKey.rawRepresentation)
        let data=try envelope(key,runtime:updates.runtime)
        let current=try NativePackage(envelope:data,app:updates.app,runtime:updates.runtime,publicKey:updates.publicKey)
        try FileManager.default.createDirectory(at:updates.directory(current),withIntermediateDirectories:true)
        let file=updates.directory(current).appendingPathComponent("index.html")
        try Data("abc".utf8).write(to:file)
        try data.write(to:home.appendingPathComponent("pending.cnu"),options:.atomic)
        XCTAssertEqual(updates.boot(),updates.directory(current)); updates.healthy()
        let same=try NativePackage(envelope:envelope(key,generation:2,runtime:updates.runtime),app:updates.app,runtime:updates.runtime,publicKey:updates.publicKey)
        XCTAssertFalse(try updates.requiresDownload(same))
        XCTAssertEqual(updates.current?.generation,1)
        XCTAssertEqual(updates.read("highwater.cnu")?.generation,2)
        XCTAssertNil(updates.pending)
        try Data("abd".utf8).write(to:file)
        let next=try NativePackage(envelope:envelope(key,generation:3,runtime:updates.runtime),app:updates.app,runtime:updates.runtime,publicKey:updates.publicKey)
        XCTAssertTrue(try updates.requiresDownload(next))
        try FileManager.default.removeItem(at:file)
        XCTAssertTrue(try updates.requiresDownload(next))
        XCTAssertEqual(updates.read("highwater.cnu")?.generation,2)
    }
    @MainActor func testActivationInterruptedTrialAndCorruptFallback() throws {
        let key=Curve25519.Signing.PrivateKey(),home=FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer {try? FileManager.default.removeItem(at:home)}
        let updates=try NativeUpdates(home:home,verificationKey:key.publicKey.rawRepresentation)
        XCTAssertEqual(updates.boot(),updates.embedded)
        for generation in 1...2 {
            let data=try envelope(key,generation:generation,runtime:updates.runtime)
            let p=try NativePackage(envelope:data,app:updates.app,runtime:updates.runtime,publicKey:updates.publicKey)
            try FileManager.default.createDirectory(at:updates.directory(p),withIntermediateDirectories:true)
            try Data("abc".utf8).write(to:updates.directory(p).appendingPathComponent("index.html"))
            try data.write(to:home.appendingPathComponent("pending.cnu"),options:.atomic)
            XCTAssertEqual(updates.boot(),updates.directory(p));XCTAssertEqual(updates.current?.generation,Int64(generation));XCTAssertTrue(updates.trial)
            if generation==1 {updates.healthy();XCTAssertFalse(updates.trial)}
        }
        // Kill before health: the second trial is quarantined, the first survives.
        _=updates.boot();XCTAssertEqual(updates.current?.generation,1)
        try Data("tampered".utf8).write(to:updates.directory(updates.current!).appendingPathComponent("index.html"))
        XCTAssertEqual(updates.boot(),updates.embedded);XCTAssertNil(updates.current)
    }
}
