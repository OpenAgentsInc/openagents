// Firm reason: 6 (independent URLSession/filesystem oracle of signed inventory delivery).
import XCTest
import CryptoKit
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
