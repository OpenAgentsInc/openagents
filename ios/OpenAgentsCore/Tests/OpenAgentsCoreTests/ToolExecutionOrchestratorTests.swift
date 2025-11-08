import XCTest
@testable import OpenAgentsCore

final class ToolExecutionOrchestratorTests: XCTestCase {
    func testExecute_emptyOps_noResults() async throws {
        let toolExec = ToolExecutor(workspaceRoot: "/tmp")
        let planner = PlanningReducer(stream: { _ in })
        let orchestrator = ToolExecutionOrchestrator(toolExecutor: toolExec, stream: { _ in }, planner: planner)
        let results = try await orchestrator.execute([])
        XCTAssertTrue(results.isEmpty)
    }
}

