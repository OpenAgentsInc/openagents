import XCTest
@testable import OpenAgentsCore

final class CodexDiscoveryTests: XCTestCase {
    func testDiscoverIncludesEnvOverride() throws {
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        setenv("CODEXD_HISTORY_DIR", tmp.path, 1)
        defer { unsetenv("CODEXD_HISTORY_DIR") }
        let found = CodexDiscovery.discoverBaseDirs(options: .init(preferEnvOnly: true))
        XCTAssertTrue(found.map{ $0.path }.contains(tmp.path), "found=\(found)")
    }

    func testContainsNewFormatJSONL() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let f = dir.appendingPathComponent("x.jsonl")
        try "{\"type\":\"thread.started\"}\n".data(using: .utf8)!.write(to: f)
        XCTAssertTrue(CodexDiscovery.containsNewFormatJSONL(in: dir))
    }

    func testLoadAllSummariesFromMultipleBases() throws {
        let base1 = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        let base2 = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: base1, withIntermediateDirectories: true)
        try FileManager.default.createDirectory(at: base2, withIntermediateDirectories: true)
        // inject env extras to include both
        setenv("CODEXD_HISTORY_DIR", base1.path, 1)
        setenv("CODEX_EXTRA_DIRS", base2.path, 1)
        defer { unsetenv("CODEXD_HISTORY_DIR"); unsetenv("CODEX_EXTRA_DIRS") }
        let f1 = base1.appendingPathComponent("a.jsonl")
        let f2 = base2.appendingPathComponent("b.jsonl")
        try "{\"type\":\"thread.started\",\"thread_id\":\"t-a\"}\n".data(using: .utf8)!.write(to: f1)
        try "{\"type\":\"thread.started\",\"thread_id\":\"t-b\"}\n".data(using: .utf8)!.write(to: f2)
        let rows = CodexDiscovery.loadAllSummaries(maxFilesPerBase: 50, maxResults: 10, options: .init(preferEnvOnly: true))
        let ids = Set(rows.map { $0.id })
        XCTAssertTrue(ids.contains("t-a"))
        XCTAssertTrue(ids.contains("t-b"))
    }

    func testRealDiscoveryDoesNotCrash() throws {
        // Optional integration: attempt discovery on the actual machine.
        // This test never fails CI; it only asserts basic invariants when env is set.
        let expect = getenv("EXPECT_CODEX_FILES") != nil
        let rows = CodexDiscovery.loadAllSummaries(maxFilesPerBase: 200, maxResults: 500)
        if expect {
            XCTAssertGreaterThan(rows.count, 0, "Expected Codex chats, found none")
        } else {
            // Always passes; useful to print for local debugging
            print("Discovered Codex summaries: \(rows.count)")
        }
    }
}
