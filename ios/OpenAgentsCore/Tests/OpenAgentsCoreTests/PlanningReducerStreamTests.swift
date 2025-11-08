import XCTest
@testable import OpenAgentsCore

final class PlanningReducerStreamTests: XCTestCase {
    func testStreamPlanAndUpdateEntry() async {
        var streamed: [ACP.Client.SessionUpdate] = []
        let reducer = PlanningReducer { update in
            streamed.append(update)
        }

        let ops: [AgentOp] = [
            AgentOp(kind: .grep(GrepParams(pattern: "TODO"))),
            AgentOp(kind: .listDir(ListDirParams(path: ".")))
        ]
        let plan = ExplorePlan(goals: ["test"], nextOps: ops)
        await reducer.streamPlan(plan)

        // Validate that a plan update was streamed with 2 entries
        guard case let .plan(p)? = streamed.last else {
            return XCTFail("Expected a plan update")
        }
        XCTAssertEqual(p.entries.count, 2)

        // Update first entry to in_progress and ensure it re-streams
        await reducer.updateEntry(opId: ops[0].opId.uuidString, to: .in_progress)
        guard case let .plan(p2)? = streamed.last else { return XCTFail("Expected a plan update after status change") }
        XCTAssertEqual(p2.entries.first?.status, .in_progress)
    }

    func testUpdateEntryAddsErrorMetadata() async {
        var streamed: [ACP.Client.SessionUpdate] = []
        let reducer = PlanningReducer { update in streamed.append(update) }

        let op = AgentOp(kind: .listDir(ListDirParams(path: ".")))
        let plan = ExplorePlan(goals: ["x"], nextOps: [op])
        await reducer.streamPlan(plan)

        // Mark completed with error
        await reducer.updateEntry(opId: op.opId.uuidString, to: .completed, error: "boom")

        guard case let .plan(p)? = streamed.last else { return XCTFail("Expected plan update") }
        let entry = try XCTUnwrap(p.entries.first)
        // Encode to JSON to inspect meta easily
        let data = try JSONEncoder().encode(p)
        let obj = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let entries = obj?["entries"] as? [[String: Any]]
        let first = entries?.first
        let meta = first?["_meta"] as? [String: Any]
        XCTAssertEqual(meta?["error"] as? String, "boom")
        XCTAssertEqual(entry.status, .completed)
    }
}
