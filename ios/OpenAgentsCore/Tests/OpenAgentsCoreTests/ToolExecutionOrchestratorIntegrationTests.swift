import XCTest
@testable import OpenAgentsCore

final class ToolExecutionOrchestratorIntegrationTests: XCTestCase {
    func testExecuteStreamsAndUpdatesPlan() async throws {
        // Create temp workspace
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("oa-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)
        let file = tmp.appendingPathComponent("hello.txt")
        try "hello\nworld\nTODO".write(to: file, atomically: true, encoding: .utf8)

        // Prepare ops and plan
        let ops: [AgentOp] = [
            AgentOp(kind: .grep(GrepParams(pattern: "hello", pathPrefix: nil, caseInsensitive: false, maxResults: 50))),
            AgentOp(kind: .listDir(ListDirParams(path: ".")))
        ]
        let plan = ExplorePlan(goals: ["test"], nextOps: ops)

        // Capture streamed updates
        var streamed: [ACP.Client.SessionUpdate] = []
        let planner = PlanningReducer { update in
            streamed.append(update)
        }
        await planner.streamPlan(plan)

        // Execute via orchestrator
        let exec = ToolExecutor(workspaceRoot: tmp.path)
        let orchestrator = ToolExecutionOrchestrator(toolExecutor: exec, stream: { update in
            streamed.append(update)
        }, planner: planner)

        let results = try await orchestrator.execute(ops)
        XCTAssertEqual(results.count, ops.count)

        // Verify that toolCall and toolCallUpdate were streamed
        let hadToolCall = streamed.contains { if case .toolCall = $0 { return true } else { return false } }
        let hadToolUpdate = streamed.contains { if case .toolCallUpdate = $0 { return true } else { return false } }
        XCTAssertTrue(hadToolCall)
        XCTAssertTrue(hadToolUpdate)
    }

    func testExecutePropagatesErrorAndStreamsErrorUpdate() async {
        let tmp = FileManager.default.temporaryDirectory.appendingPathComponent("oa-tests-\(UUID().uuidString)")
        try? FileManager.default.createDirectory(at: tmp, withIntermediateDirectories: true)

        // Non-existent file to force failure
        let bad = AgentOp(kind: .readSpan(ReadSpanParams(path: "nope.txt", startLine: 1, endLine: 10)))
        let plan = ExplorePlan(goals: ["test"], nextOps: [bad])

        var streamed: [ACP.Client.SessionUpdate] = []
        let planner = PlanningReducer { update in streamed.append(update) }
        await planner.streamPlan(plan)

        let exec = ToolExecutor(workspaceRoot: tmp.path)
        let orchestrator = ToolExecutionOrchestrator(toolExecutor: exec, stream: { update in streamed.append(update) }, planner: planner)

        do {
            _ = try await orchestrator.execute([bad])
            XCTFail("Expected error to be thrown")
        } catch {
            // ok
        }

        // Ensure an error update was streamed
        let hadError = streamed.contains { update in
            if case let .toolCallUpdate(u) = update { return u.status == .error } else { return false }
        }
        XCTAssertTrue(hadError)
    }
}

