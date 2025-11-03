import XCTest
@testable import OpenAgentsCore

final class CodexScannerTests: XCTestCase {
    func write(_ url: URL, _ lines: [String]) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let joined = lines.joined(separator: "\n") + "\n"
        try joined.data(using: .utf8)!.write(to: url)
    }

    func testExtractThreadIDFromSessionMeta() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let file = dir.appendingPathComponent("rollout-test.jsonl")
        try write(file, [
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"abc-123\"}}",
            "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"hi\"}}"
        ])
        let tid = CodexScanner.scanForThreadID(file)
        XCTAssertEqual(tid, "abc-123")
    }

    func testDerivesUUIDFromFilename() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let file = dir.appendingPathComponent("rollout-2025-10-22T12-05-17-019a0ce1-d491-76d2-93ba-0d47dde32657.jsonl")
        try write(file, ["{\"type\":\"event_msg\",\"payload\":{}}"])
        let tid = CodexScanner.scanForThreadID(file)
        XCTAssertEqual(tid, "019a0ce1-d491-76d2-93ba-0d47dde32657")
    }

    func testScanSummariesFromTempDir() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let file = base.appendingPathComponent("a.jsonl")
        try write(file, [
            "{\"type\":\"thread.started\",\"thread_id\":\"t-1\"}",
            "{\"type\":\"item.completed\",\"payload\":{\"ts\": 1234567890}}"
        ])
        let rows = CodexScanner.scan(options: .init(baseDir: base, maxFiles: 50))
        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(rows[0].id, "t-1")
        XCTAssertEqual(rows[0].source, "codex")
        XCTAssertNotEqual(rows[0].updated_at, 0)
    }
}
