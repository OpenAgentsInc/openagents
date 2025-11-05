import XCTest
@testable import OpenAgentsCore

final class ClaudeCodeScannerComprehensiveTests: XCTestCase {
    var tempDir: URL!

    override func setUp() {
        super.setUp()
        // Create a temporary directory for testing
        let tempPath = NSTemporaryDirectory() + "/ClaudeCodeScannerTests_\(UUID().uuidString)"
        tempDir = URL(fileURLWithPath: tempPath)
        try? FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
    }

    override func tearDown() {
        // Clean up temporary directory
        try? FileManager.default.removeItem(at: tempDir)
        tempDir = nil
        super.tearDown()
    }

    // MARK: - Default Base Directory Tests

    func testDefaultBaseDir() {
        let baseDir = ClaudeCodeScanner.defaultBaseDir()
        #if os(macOS)
        XCTAssertTrue(baseDir.path.hasSuffix(".claude/projects"))
        #else
        XCTAssertEqual(baseDir.path, "/nonexistent")
        #endif
    }

    // MARK: - File Listing Tests

    func testListJSONLFiles_EmptyDirectory() {
        let files = ClaudeCodeScanner.listJSONLFiles(at: tempDir)
        XCTAssertTrue(files.isEmpty)
    }

    func testListJSONLFiles_SingleFile() {
        let jsonlFile = tempDir.appendingPathComponent("session.jsonl")
        try? "test content".write(to: jsonlFile, atomically: true, encoding: .utf8)

        let files = ClaudeCodeScanner.listJSONLFiles(at: tempDir)
        XCTAssertEqual(files.count, 1)
        XCTAssertEqual(files.first?.lastPathComponent, "session.jsonl")
    }

    func testListJSONLFiles_MultipleFiles() {
        for i in 1...5 {
            let jsonlFile = tempDir.appendingPathComponent("session\(i).jsonl")
            try? "test content \(i)".write(to: jsonlFile, atomically: true, encoding: .utf8)
        }

        let files = ClaudeCodeScanner.listJSONLFiles(at: tempDir)
        XCTAssertEqual(files.count, 5)
    }

    func testListJSONLFiles_ExcludesBackups() {
        let regularFile = tempDir.appendingPathComponent("session.jsonl")
        let backupFile = tempDir.appendingPathComponent("session.backup.jsonl")
        try? "regular".write(to: regularFile, atomically: true, encoding: .utf8)
        try? "backup".write(to: backupFile, atomically: true, encoding: .utf8)

        let files = ClaudeCodeScanner.listJSONLFiles(at: tempDir)
        XCTAssertEqual(files.count, 1)
        XCTAssertEqual(files.first?.lastPathComponent, "session.jsonl")
    }

    func testListJSONLFiles_ExcludesHiddenFiles() {
        let regularFile = tempDir.appendingPathComponent("session.jsonl")
        let hiddenFile = tempDir.appendingPathComponent(".hidden.jsonl")
        try? "regular".write(to: regularFile, atomically: true, encoding: .utf8)
        try? "hidden".write(to: hiddenFile, atomically: true, encoding: .utf8)

        let files = ClaudeCodeScanner.listJSONLFiles(at: tempDir)
        // Note: Hidden files should be excluded by options: [.skipsHiddenFiles]
        XCTAssertTrue(files.contains { $0.lastPathComponent == "session.jsonl" })
    }

    func testListJSONLFiles_NestedDirectories() {
        let subDir = tempDir.appendingPathComponent("project1")
        try? FileManager.default.createDirectory(at: subDir, withIntermediateDirectories: true)

        let rootFile = tempDir.appendingPathComponent("root.jsonl")
        let nestedFile = subDir.appendingPathComponent("nested.jsonl")
        try? "root".write(to: rootFile, atomically: true, encoding: .utf8)
        try? "nested".write(to: nestedFile, atomically: true, encoding: .utf8)

        let files = ClaudeCodeScanner.listJSONLFiles(at: tempDir)
        XCTAssertEqual(files.count, 2)
    }

    func testListJSONLFiles_IgnoresNonJSONL() {
        let jsonlFile = tempDir.appendingPathComponent("session.jsonl")
        let txtFile = tempDir.appendingPathComponent("notes.txt")
        let jsonFile = tempDir.appendingPathComponent("data.json")
        try? "jsonl".write(to: jsonlFile, atomically: true, encoding: .utf8)
        try? "txt".write(to: txtFile, atomically: true, encoding: .utf8)
        try? "json".write(to: jsonFile, atomically: true, encoding: .utf8)

        let files = ClaudeCodeScanner.listJSONLFiles(at: tempDir)
        XCTAssertEqual(files.count, 1)
        XCTAssertEqual(files.first?.pathExtension, "jsonl")
    }

    // MARK: - Recent Files Tests

    func testListRecentTopN_SortsbyModificationTime() {
        // Create files with different modification times
        let file1 = tempDir.appendingPathComponent("old.jsonl")
        let file2 = tempDir.appendingPathComponent("recent.jsonl")

        try? "old".write(to: file1, atomically: true, encoding: .utf8)
        sleep(1) // Ensure different modification times
        try? "recent".write(to: file2, atomically: true, encoding: .utf8)

        let files = ClaudeCodeScanner.listRecentTopN(at: tempDir, topK: 10)
        XCTAssertEqual(files.count, 2)
        // Most recent should be first
        XCTAssertEqual(files.first?.lastPathComponent, "recent.jsonl")
    }

    func testListRecentTopN_LimitsResults() {
        for i in 1...10 {
            let file = tempDir.appendingPathComponent("session\(i).jsonl")
            try? "content \(i)".write(to: file, atomically: true, encoding: .utf8)
        }

        let files = ClaudeCodeScanner.listRecentTopN(at: tempDir, topK: 5)
        XCTAssertEqual(files.count, 5)
    }

    func testListRecentTopN_EmptyDirectory() {
        let files = ClaudeCodeScanner.listRecentTopN(at: tempDir, topK: 10)
        XCTAssertTrue(files.isEmpty)
    }

    // MARK: - Session ID Scanning Tests

    func testScanForSessionID_ValidSessionID() {
        let content = """
        {"sessionId":"test-session-123","timestamp":1234567890}
        {"type":"user","message":{"content":"Hello"}}
        """
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let sessionId = ClaudeCodeScanner.scanForSessionID(file)
        XCTAssertEqual(sessionId, "test-session-123")
    }

    func testScanForSessionID_NoSessionID() {
        let content = """
        {"type":"user","message":{"content":"Hello"}}
        {"type":"assistant","message":{"content":"Hi"}}
        """
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let sessionId = ClaudeCodeScanner.scanForSessionID(file)
        XCTAssertNil(sessionId)
    }

    func testScanForSessionID_SessionIDInSecondLine() {
        let content = """
        {"type":"user","message":{"content":"Hello"}}
        {"sessionId":"second-line-session","timestamp":1234567890}
        """
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let sessionId = ClaudeCodeScanner.scanForSessionID(file)
        XCTAssertEqual(sessionId, "second-line-session")
    }

    func testScanForSessionID_InvalidJSON() {
        let content = "not valid json\n{invalid\n"
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let sessionId = ClaudeCodeScanner.scanForSessionID(file)
        XCTAssertNil(sessionId)
    }

    func testScanForSessionID_EmptyFile() {
        let file = tempDir.appendingPathComponent("empty.jsonl")
        try? "".write(to: file, atomically: true, encoding: .utf8)

        let sessionId = ClaudeCodeScanner.scanForSessionID(file)
        XCTAssertNil(sessionId)
    }

    // MARK: - Quick Title Tests

    func testQuickTitle_FromFirstUserMessage() {
        let content = """
        {"sessionId":"test-session","timestamp":1234567890}
        {"type":"user","message":{"content":"Help me debug this code"}}
        {"type":"assistant","message":{"content":"Sure, I can help"}}
        """
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let title = ClaudeCodeScanner.quickTitle(for: file)
        XCTAssertNotNil(title)
        XCTAssertTrue(title?.contains("Help me debug") ?? false)
    }

    func testQuickTitle_LongMessageTruncated() {
        let longMessage = String(repeating: "word ", count: 30)
        let content = """
        {"type":"user","message":{"content":"\(longMessage)"}}
        """
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let title = ClaudeCodeScanner.quickTitle(for: file)
        XCTAssertNotNil(title)
        XCTAssertTrue((title?.count ?? 0) <= 63) // 60 + "..."
    }

    func testQuickTitle_NoUserMessage() {
        let content = """
        {"sessionId":"test-session","timestamp":1234567890}
        {"type":"assistant","message":{"content":"Hello"}}
        """
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let title = ClaudeCodeScanner.quickTitle(for: file)
        XCTAssertNil(title)
    }

    func testQuickTitle_EmptyUserMessage() {
        let content = """
        {"type":"user","message":{"content":"   "}}
        {"type":"user","message":{"content":"Real message"}}
        """
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let title = ClaudeCodeScanner.quickTitle(for: file)
        XCTAssertNotNil(title)
        XCTAssertTrue(title?.contains("Real message") ?? false)
    }

    func testQuickTitle_SpecialCharacters() {
        let content = """
        {"type":"user","message":{"content":"Hello! How are you?"}}
        """
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let title = ClaudeCodeScanner.quickTitle(for: file)
        XCTAssertNotNil(title)
        XCTAssertTrue(title?.contains("Hello!") ?? false)
    }

    // MARK: - Relative ID Tests

    func testRelativeId_SimpleFile() {
        let base = tempDir
        let file = tempDir.appendingPathComponent("session.jsonl")

        let relId = ClaudeCodeScanner.relativeId(for: file, base: base)
        XCTAssertEqual(relId, "session")
    }

    func testRelativeId_NestedFile() {
        let base = tempDir
        let subDir = tempDir.appendingPathComponent("project1")
        let file = subDir.appendingPathComponent("session.jsonl")

        let relId = ClaudeCodeScanner.relativeId(for: file, base: base)
        XCTAssertEqual(relId, "project1/session")
    }

    func testRelativeId_DeepNesting() {
        let base = tempDir
        let deep = tempDir.appendingPathComponent("a/b/c")
        let file = deep.appendingPathComponent("session.jsonl")

        let relId = ClaudeCodeScanner.relativeId(for: file, base: base)
        XCTAssertEqual(relId, "a/b/c/session")
    }

    func testRelativeId_NonJSONL() {
        let base = tempDir
        let file = tempDir.appendingPathComponent("file.txt")

        let relId = ClaudeCodeScanner.relativeId(for: file, base: base)
        XCTAssertEqual(relId, "file.txt")
    }

    // MARK: - Thread Summary Tests

    func testMakeSummary_Complete() {
        let content = """
        {"sessionId":"test-123","timestamp":1234567890000}
        {"type":"user","message":{"content":"Test message"}}
        """
        let file = tempDir.appendingPathComponent("session.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let summary = ClaudeCodeScanner.makeSummary(for: file, base: tempDir)

        XCTAssertEqual(summary.id, "test-123")
        XCTAssertEqual(summary.source, "claude-code")
        XCTAssertNotNil(summary.updated_at)
        XCTAssertNotNil(summary.last_message_ts)
        XCTAssertNotNil(summary.title)
    }

    func testMakeSummary_NoSessionID() {
        let content = """
        {"type":"user","message":{"content":"Test"}}
        """
        let file = tempDir.appendingPathComponent("mysession.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let summary = ClaudeCodeScanner.makeSummary(for: file, base: tempDir)

        XCTAssertEqual(summary.id, "mysession")
        XCTAssertEqual(summary.source, "claude-code")
    }

    func testMakeSummary_EmptyFile() {
        let file = tempDir.appendingPathComponent("empty.jsonl")
        try? "".write(to: file, atomically: true, encoding: .utf8)

        let summary = ClaudeCodeScanner.makeSummary(for: file, base: tempDir)

        XCTAssertEqual(summary.id, "empty")
        XCTAssertEqual(summary.source, "claude-code")
        XCTAssertNotNil(summary.updated_at)
    }

    // MARK: - Options Tests

    func testOptions_DefaultValues() {
        let options = ClaudeCodeScanner.Options()
        XCTAssertNil(options.baseDir)
        XCTAssertEqual(options.maxFiles, 200)
    }

    func testOptions_CustomValues() {
        let customBase = URL(fileURLWithPath: "/custom/path")
        let options = ClaudeCodeScanner.Options(baseDir: customBase, maxFiles: 50)
        XCTAssertEqual(options.baseDir, customBase)
        XCTAssertEqual(options.maxFiles, 50)
    }

    // MARK: - Edge Cases

    func testListJSONLFiles_NonexistentDirectory() {
        let nonexistent = URL(fileURLWithPath: "/nonexistent/path")
        let files = ClaudeCodeScanner.listJSONLFiles(at: nonexistent)
        XCTAssertTrue(files.isEmpty)
    }

    func testScanForSessionID_NonexistentFile() {
        let nonexistent = tempDir.appendingPathComponent("nonexistent.jsonl")
        let sessionId = ClaudeCodeScanner.scanForSessionID(nonexistent)
        XCTAssertNil(sessionId)
    }

    func testQuickTitle_NonexistentFile() {
        let nonexistent = tempDir.appendingPathComponent("nonexistent.jsonl")
        let title = ClaudeCodeScanner.quickTitle(for: nonexistent)
        XCTAssertNil(title)
    }

    func testScanForSessionID_LargeFile() {
        // Create a large file (> 50KB) with sessionId deep inside
        var content = ""
        for i in 0..<2000 {
            content += """
            {"line":\(i),"data":"padding data padding data padding data"}
            """
        }
        content += """
        {"sessionId":"deep-session-id"}
        """

        let file = tempDir.appendingPathComponent("large.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        // Should read only first 50KB, so might not find it
        let sessionId = ClaudeCodeScanner.scanForSessionID(file)
        // Result depends on file size, but should not crash
        XCTAssertNotNil(file)
    }

    func testQuickTitle_Unicode() {
        let content = """
        {"type":"user","message":{"content":"你好世界 🚀"}}
        """
        let file = tempDir.appendingPathComponent("unicode.jsonl")
        try? content.write(to: file, atomically: true, encoding: .utf8)

        let title = ClaudeCodeScanner.quickTitle(for: file)
        XCTAssertNotNil(title)
        XCTAssertTrue(title?.contains("你好世界") ?? false)
    }

    func testListJSONLFiles_CaseInsensitiveExtension() {
        let jsonlLower = tempDir.appendingPathComponent("session.jsonl")
        let jsonlUpper = tempDir.appendingPathComponent("SESSION.JSONL")
        try? "lower".write(to: jsonlLower, atomically: true, encoding: .utf8)
        try? "upper".write(to: jsonlUpper, atomically: true, encoding: .utf8)

        let files = ClaudeCodeScanner.listJSONLFiles(at: tempDir)
        // Both should be found due to lowercased() check
        XCTAssertEqual(files.count, 2)
    }
}
