import XCTest
@testable import OpenAgentsCore

final class PlanningReducerTests: XCTestCase {
    func testAddAnalysisIfNeeded_appendsAnalyzeWhenSessionOpsPresent() {
        let ops: [AgentOp] = [
            AgentOp(kind: .sessionList(SessionListParams(provider: nil, topK: 10, since: nil))),
            AgentOp(kind: .grep(GrepParams(pattern: "TODO", pathPrefix: nil, caseInsensitive: false, maxResults: 10)))
        ]
        let out = PlanningReducer.addAnalysisIfNeeded(ops)
        XCTAssertGreaterThan(out.count, ops.count)
        XCTAssertTrue(out.contains { if case .sessionAnalyze = $0.kind { return true } else { return false } })
    }

    func testParseOperationsFromResponse_basic() throws {
        let response = """
        sessionList claude-code
        grep "TODO"
        """
        let ops = try PlanningReducer.parseOperationsFromResponse(response)
        XCTAssertGreaterThan(ops.count, 0)
    }
}

