import XCTest
@testable import OpenAgentsCore

final class HistoryApiTests: XCTestCase {
    func testRecentSessions_EmptyDB_ReturnsEmpty() async throws {
        #if os(macOS)
        let tmp = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("HistoryApiEmpty-").appendingPathExtension("sqlite")
        let db = try TinyvexDbLayer(path: tmp.path)
        let api = HistoryApi(tinyvexDb: db)
        let items = try await api.recentSessions()
        XCTAssertEqual(items.count, 0)
        #else
        throw XCTSkip("macOS-only")
        #endif
    }

    func testSessionTimeline_InvalidId_Empty() async throws {
        #if os(macOS)
        let tmp = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("HistoryApiInvalid-").appendingPathExtension("sqlite")
        let db = try TinyvexDbLayer(path: tmp.path)
        let api = HistoryApi(tinyvexDb: db)
        let arr = try await api.sessionTimeline(sessionId: "does-not-exist", limit: nil)
        XCTAssertEqual(arr.count, 0)
        #else
        throw XCTSkip("macOS-only")
        #endif
    }

    func testRecentSessions_NoDB_Throws() async throws {
        #if os(macOS)
        let api = HistoryApi(tinyvexDb: nil)
        await XCTAssertThrowsErrorAsync(try await api.recentSessions())
        #else
        throw XCTSkip("macOS-only")
        #endif
    }
}

extension XCTestCase {
    func XCTAssertThrowsErrorAsync<T>(_ expression: @autoclosure @escaping () async throws -> T,
                                      _ message: @autoclosure () -> String = "",
                                      file: StaticString = #filePath, line: UInt = #line) async {
        do { _ = try await expression(); XCTFail(message(), file: file, line: line) } catch { /* expected */ }
    }
}

