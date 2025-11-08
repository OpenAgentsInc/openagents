import XCTest
@testable import OpenAgentsCore

final class SummaryBuilderTests: XCTestCase {
    func testGenerateSummaryAggregatesSessionsAndFiles() async {
        let workspace = "/tmp/myrepo"

        var opResults: [AgentOp: any Encodable] = [:]

        // session.list result
        let listOp = AgentOp(kind: .sessionList(SessionListParams(provider: "claude-code", topK: 2)))
        let sessA = SessionMetadata(id: "s1", title: "Fix login", provider: "claude-code", updated_at: 1000, file_path: "/a.jsonl")
        let sessB = SessionMetadata(id: "s2", title: "Warmup", provider: "claude-code", updated_at: 2000, file_path: "/b.jsonl")
        let listRes = SessionListResult(sessions: [sessA, sessB], truncated: false, total_count: 2)
        opResults[listOp] = listRes

        // session.read result with file references
        let readOp = AgentOp(kind: .sessionRead(SessionReadParams(sessionId: "s1", provider: "claude-code")))
        let events = [SessionEvent(type: "user", lineNumber: 1, content: "hi")]
        let readRes = SessionReadResult(sessionId: "s1", events: events, truncated: false, totalEvents: 1, fileReferences: ["README.md"])
        opResults[readOp] = readRes

        // grep result
        let grepOp = AgentOp(kind: .grep(GrepParams(pattern: "TODO", pathPrefix: nil)))
        let matches = [GrepMatch(path: "src/App.swift", line_number: 1, line: "// TODO")] 
        let grepRes = GrepResult(pattern: "TODO", matches: matches, truncated: false, total_matches: 1)
        opResults[grepOp] = grepRes

        let summary = await SummaryBuilder.generate(workspaceRoot: workspace, operationResults: opResults)
        XCTAssertEqual(summary.repo_name, "myrepo")
        XCTAssertTrue(summary.top_files.contains("README.md") || summary.top_files.contains("src/App.swift"))
        XCTAssertFalse(summary.followups.isEmpty)
    }
}

