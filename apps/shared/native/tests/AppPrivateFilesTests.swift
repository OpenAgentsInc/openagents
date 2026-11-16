// Firm reason: 6 (controlled macOS SDK filesystem qualification; no real account data).
#if os(macOS)
import XCTest
import Foundation

@MainActor final class AppPrivateFilesTests: XCTestCase {
    func testViewCodecAndAtomicStoreKeepOnlyPublicState() throws {
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("oa-view-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: folder) }
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o755])
        let file = folder.appendingPathComponent("native-view-v1.json"), store = AppFileViewStore(url: file)
        XCTAssertEqual(store.load(), AppViewState())
        let value = AppViewState(email: "café@example.test", rejoin: "host\tsession", environment: "draft\t-\trepo\n")
        XCTAssertTrue(store.save(value)); XCTAssertEqual(store.load(), value)
        let bytes = try Data(contentsOf: file)
        XCTAssertTrue(String(decoding: bytes, as: UTF8.self).hasPrefix("{\"schema\":\"openagents.account.native-view.v1\",\"email\":"))
        XCTAssertEqual((try FileManager.default.attributesOfItem(atPath: file.path)[.posixPermissions] as? NSNumber)?.intValue, 0o600)
        XCTAssertEqual((try FileManager.default.attributesOfItem(atPath: folder.path)[.posixPermissions] as? NSNumber)?.intValue, 0o700)
        try Data("{\"schema\":\"openagents.account.native-view.v1\",\"email\":\"partial\"}".utf8).write(to: file)
        XCTAssertEqual(store.load(), AppViewState())
        XCTAssertFalse(store.save(AppViewState(email: String(repeating: "x", count: 321))))
    }
    func testWorkspacePendingCapacityIsExplicitAndExclusive() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("oa-workspace-pending-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppFileIdentityPending(folder: root, limit: 24576)
        let exact = Data(repeating: 120, count: 24576)
        XCTAssertTrue(store.save(exact, account: "fixture")); XCTAssertEqual(store.load("fixture"), exact)
        XCTAssertFalse(store.save(Data(), account: "fixture")); store.remove("fixture")
        XCTAssertFalse(store.save(Data(repeating: 120, count: 24577), account: "fixture"))
        XCTAssertNil(store.load("fixture"))
    }
    func testPrivateFileExclusiveWritesRefuseReplacementAndSymlinks() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("oa-private-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: root) }
        let file = AppPrivateFile(url: root.appendingPathComponent("pending"), limit: 32)
        let first = Data("disposable fixture".utf8), second = Data("changed fixture".utf8)
        XCTAssertTrue(file.create(first)); XCTAssertFalse(file.create(second)); XCTAssertEqual(file.load(), first)
        XCTAssertFalse(file.create(Data(repeating: 0, count: 33)))
        file.remove(); XCTAssertNil(file.load())
        let target = root.appendingPathComponent("target"), link = root.appendingPathComponent("link")
        try first.write(to: target); try FileManager.default.createSymbolicLink(at: link, withDestinationURL: target)
        let linked = AppPrivateFile(url: link, limit: 32)
        XCTAssertNil(linked.load()); XCTAssertFalse(linked.replace(second)); XCTAssertFalse(linked.create(second))
        XCTAssertEqual(try Data(contentsOf: target), first)
    }
}
#endif
