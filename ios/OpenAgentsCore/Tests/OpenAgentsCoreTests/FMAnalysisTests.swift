import XCTest
@testable import OpenAgentsCore

final class FMAnalysisTests: XCTestCase {
    #if canImport(FoundationModels)
    @available(iOS 26.0, macOS 26.0, *)
    func testComputeFromAnalyzeWithBulletedLabel() async throws {
        // Given a userIntent with a label and bullets
        let analyze = SessionAnalyzeResult(
            fileFrequency: ["/Users/me/repo/README.md": 3, "/Users/me/repo/src/App.swift": 2],
            toolFrequency: nil,
            goalPatterns: ["cleanup", "refactor"],
            avgConversationLength: 12.0,
            userIntent: """
            Read:
            - README.md
            - src/App.swift
            """
        )

        let result = await FMAnalysis.compute(workspaceRoot: "/Users/me/repo", operationResults: [
            AgentOp(kind: .sessionAnalyze(SessionAnalyzeParams(sessionIds: [], provider: nil))): analyze
        ])

        // Then we get a deterministic sentence and normalized top files
        XCTAssertNotNil(result)
        let r = try XCTUnwrap(result)
        XCTAssertTrue(r.text.lowercased().contains("user intends to read"))
        XCTAssertTrue(r.topFiles.contains("README.md") || r.topFiles.contains("src/App.swift"))
        XCTAssertEqual(r.source, .sessionAnalyze)
    }

    @available(iOS 26.0, macOS 26.0, *)
    func testComputeFromAnalyzePlainText() async throws {
        let analyze = SessionAnalyzeResult(
            fileFrequency: ["/Users/me/repo/Models/User.swift": 4],
            toolFrequency: nil,
            goalPatterns: ["auth"],
            avgConversationLength: 7.0,
            userIntent: "Fix login bug by adjusting validation"
        )
        let result = await FMAnalysis.compute(workspaceRoot: "/Users/me/repo", operationResults: [
            AgentOp(kind: .sessionAnalyze(SessionAnalyzeParams(sessionIds: [], provider: nil))): analyze
        ])
        XCTAssertNotNil(result)
        let r = try XCTUnwrap(result)
        XCTAssertTrue(r.text.lowercased().contains("fix login bug"))
        XCTAssertEqual(r.source, .sessionAnalyze)
    }
    #else
    func testComputeUnavailableReturnsNil() async throws {
        let result = await FMAnalysis.compute(workspaceRoot: "/tmp", operationResults: [:])
        XCTAssertNil(result)
    }
    #endif
}

