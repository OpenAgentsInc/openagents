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
        XCTAssertTrue(rows[0].id.hasSuffix("sess-123"))
        XCTAssertEqual(rows[0].source, "claude_code")
    }

    func testDistinctIdsForSameStemDifferentDirs() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let p1 = base.appendingPathComponent("projA/transcript.jsonl")
        let p2 = base.appendingPathComponent("projB/transcript.jsonl")
        try write(p1, ["{\"type\":\"assistant\",\"message\":{\"content\":[],\"ts\": 1000}}"])
        try write(p2, ["{\"type\":\"assistant\",\"message\":{\"content\":[],\"ts\": 2000}}"])
        let rows = ClaudeScanner.scan(options: .init(baseDir: base, maxFiles: 50))
        XCTAssertEqual(rows.count, 2, "rows=\(rows)")
        let ids = Set(rows.map { $0.id })
        XCTAssertTrue(ids.contains("projA/transcript"), "ids=\(ids)")
        XCTAssertTrue(ids.contains("projB/transcript"), "ids=\(ids)")
    }

    func testScanTopKReturnsAtMost10() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        for i in 0..<15 {
            let f = base.appendingPathComponent("p\(i)/s\(i).jsonl")
            try write(f, ["{\"type\":\"assistant\",\"message\":{\"content\":[],\"ts\": \(1000 + i)}}"])
        }
        let rows = ClaudeScanner.scanTopK(options: .init(baseDir: base, maxFiles: 100), topK: 10)
        XCTAssertEqual(rows.count, 10)
        // Ensure descending order by updated_at
        let times = rows.map { $0.updated_at }
        XCTAssertEqual(times, times.sorted(by: >))
    }
}
