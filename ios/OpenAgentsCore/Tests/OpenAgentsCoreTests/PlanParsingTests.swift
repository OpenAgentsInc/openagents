import XCTest
@testable import OpenAgentsCore

final class PlanParsingTests: XCTestCase {
    func testParseSessionAndFileOps() {
        let fmText = """
        sessionList claude-code
        sessionSearch "TODO"
        sessionRead session abc123
        sessionAnalyze
        Read README.md 1-50
        grep "import" in src
        listDir src
        """

        let ops = PlanParsing.parseOperationsFromResponse(fmText, workspaceRoot: "/Users/me/repo")
        XCTAssertFalse(ops.isEmpty)
        // Expect at least one of each kind parsed
        XCTAssertTrue(ops.contains { if case .sessionList = $0.kind { return true } else { return false } })
        XCTAssertTrue(ops.contains { if case .sessionSearch = $0.kind { return true } else { return false } })
        XCTAssertTrue(ops.contains { if case .sessionRead = $0.kind { return true } else { return false } })
        XCTAssertTrue(ops.contains { if case .sessionAnalyze = $0.kind { return true } else { return false } })
        XCTAssertTrue(ops.contains { if case .readSpan = $0.kind { return true } else { return false } })
        XCTAssertTrue(ops.contains { if case .grep = $0.kind { return true } else { return false } })
        XCTAssertTrue(ops.contains { if case .listDir = $0.kind { return true } else { return false } })
    }
}

