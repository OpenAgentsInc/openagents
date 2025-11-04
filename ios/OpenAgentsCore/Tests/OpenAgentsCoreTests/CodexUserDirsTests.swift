import XCTest
@testable import OpenAgentsCore

final class CodexUserDirsTests: XCTestCase {
    func testUserSessions2025IfPresent() throws {
        let path = "/Users/christopherdavid/.codex/sessions/2025"
        let url = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: url.path) else {
            print("User sessions dir not present: \(path). Skipping.")
            return
        }
        // Allow assertion only when explicitly enabled to avoid CI variability.
        let assertEnabled = getenv("EXPECT_USER_CODEX") != nil
        let rows = CodexScanner.scan(options: .init(baseDir: url, maxFiles: 5000))
        print("Scan of \(path) returned \(rows.count) summaries")
        if assertEnabled {
            XCTAssertGreaterThan(rows.count, 1, "Expected multiple chats under \(path)")
        }
    }
}

