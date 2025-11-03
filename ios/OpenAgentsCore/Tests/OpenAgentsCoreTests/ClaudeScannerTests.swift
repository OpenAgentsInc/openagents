import XCTest
@testable import OpenAgentsCore

final class ClaudeScannerTests: XCTestCase {
    func write(_ url: URL, _ lines: [String]) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let joined = lines.joined(separator: "\n") + "\n"
        try joined.data(using: .utf8)!.write(to: url)
    }

    func testSessionIdFromFilenameStem() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let file = dir.appendingPathComponent("sess-123.jsonl")
        try write(file, ["{\"type\":\"assistant\",\"message\":{\"content\":[],\"ts\": 1000}}"])
        let rows = ClaudeScanner.scan(options: .init(baseDir: dir, maxFiles: 50))
        XCTAssertEqual(rows.count, 1)
        XCTAssertEqual(rows[0].id, "sess-123")
        XCTAssertEqual(rows[0].source, "claude_code")
    }
}
