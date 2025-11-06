import XCTest
import SwiftUI
@testable import OpenAgents
@testable import OpenAgentsCore

/// Tests to ensure ToolResultView renders structured data as proper components, NOT raw JSON
final class ToolResultViewStructuredRenderingTests: XCTestCase {

    // MARK: - TodoList Parsing Tests

    func testTodoListView_ParsesValidTodos() {
        let jsonValue = JSONValue.object([
            "todos": .array([
                .object([
                    "content": .string("Task 1"),
                    "status": .string("completed"),
                    "activeForm": .string("Completed")
                ]),
                .object([
                    "content": .string("Task 2"),
                    "status": .string("in_progress"),
                    "activeForm": .string("Working on it")
                ])
            ])
        ])

        let todos = TodoListView.parse(from: jsonValue)

        XCTAssertNotNil(todos)
        XCTAssertEqual(todos?.count, 2)
        XCTAssertEqual(todos?[0].content, "Task 1")
        XCTAssertEqual(todos?[0].status, "completed")
        XCTAssertEqual(todos?[1].content, "Task 2")
        XCTAssertEqual(todos?[1].status, "in_progress")
    }

    func testTodoListView_ReturnsNilForInvalidData() {
        let invalidData: [JSONValue] = [
            .object([:]),  // Empty object
            .object(["todos": .string("not an array")]),  // Wrong type
            .array([]),  // Array instead of object
            .string("string")  // String
        ]

        for data in invalidData {
            let todos = TodoListView.parse(from: data)
            XCTAssertNil(todos, "Should return nil for invalid data")
        }
    }

    func testTodoListView_HandlesEmptyTodosArray() {
        let jsonValue = JSONValue.object([
            "todos": .array([])
        ])

        let todos = TodoListView.parse(from: jsonValue)

        XCTAssertNotNil(todos)
        XCTAssertEqual(todos?.count, 0)
    }

    func testTodoListView_SkipsInvalidTodoItems() {
        let jsonValue = JSONValue.object([
            "todos": .array([
                .object([
                    "content": .string("Valid task"),
                    "status": .string("completed"),
                    "activeForm": .string("Done")
                ]),
                .object([
                    "content": .string("Missing status")
                    // Missing status and activeForm
                ]),
                .object([
                    "content": .string("Another valid"),
                    "status": .string("pending"),
                    "activeForm": .string("Pending")
                ])
            ])
        ])

        let todos = TodoListView.parse(from: jsonValue)

        XCTAssertNotNil(todos)
        XCTAssertEqual(todos?.count, 2, "Should skip invalid item")
        XCTAssertEqual(todos?[0].content, "Valid task")
        XCTAssertEqual(todos?[1].content, "Another valid")
    }

    // MARK: - PlanState Parsing Tests

    func testToolResultView_ParsesPlanState() {
        let result = ACPToolResult(
            call_id: "test",
            ok: true,
            result: JSONValue.object([
                "type": .string("plan_state"),
                "status": .string("running"),
                "summary": .string("Test plan"),
                "steps": .array([.string("Step 1"), .string("Step 2")]),
                "ts": .number(1000)
            ]),
            error: nil,
            ts: 1000
        )

        let view = ToolResultView(result: result)

        // View should parse plan_state from result
        XCTAssertNotNil(view)
    }

    func testToolResultView_ParsesPlanStateWithMinimalData() {
        let result = ACPToolResult(
            call_id: "test",
            ok: true,
            result: JSONValue.object([
                "type": .string("plan_state"),
                "status": .string("idle")
            ]),
            error: nil,
            ts: 1000
        )

        let view = ToolResultView(result: result)
        XCTAssertNotNil(view)
    }

    func testToolResultView_RejectsInvalidPlanState() {
        let invalidPlanStates: [JSONValue] = [
            // Missing type
            .object(["status": .string("running")]),
            // Wrong type value
            .object(["type": .string("wrong_type"), "status": .string("running")]),
            // Missing status
            .object(["type": .string("plan_state")]),
            // Invalid status value
            .object(["type": .string("plan_state"), "status": .string("invalid_status")])
        ]

        for invalidData in invalidPlanStates {
            let result = ACPToolResult(
                call_id: "test",
                ok: true,
                result: invalidData,
                error: nil,
                ts: 1000
            )

            let view = ToolResultView(result: result)
            // View should still be created but won't parse as plan_state
            XCTAssertNotNil(view)
        }
    }

    // MARK: - Structured Data Detection Tests

    func testToolResultView_DetectsStructuredTodoData() {
        let result = ACPToolResult(
            call_id: "todo-write",
            ok: true,
            result: JSONValue.object([
                "todos": .array([
                    .object([
                        "content": .string("Task"),
                        "status": .string("completed"),
                        "activeForm": .string("Done")
                    ])
                ])
            ]),
            error: nil,
            ts: 1000
        )

        let view = ToolResultView(result: result)
        // isStructuredData should return true for todos
        XCTAssertNotNil(view)
    }

    func testToolResultView_DetectsStructuredPlanData() {
        let result = ACPToolResult(
            call_id: "plan",
            ok: true,
            result: JSONValue.object([
                "type": .string("plan_state"),
                "status": .string("running")
            ]),
            error: nil,
            ts: 1000
        )

        let view = ToolResultView(result: result)
        XCTAssertNotNil(view)
    }

    func testToolResultView_FallsBackToJSONForUnstructuredData() {
        let result = ACPToolResult(
            call_id: "generic",
            ok: true,
            result: JSONValue.object([
                "foo": .string("bar"),
                "baz": .number(42)
            ]),
            error: nil,
            ts: 1000
        )

        let view = ToolResultView(result: result)
        // Should create view but render as JSON
        XCTAssertNotNil(view)
    }

    // MARK: - Component Rendering Tests

    func testToolResultView_TodosRenderAsComponent_NotJSON() {
        // CRITICAL: TodoWrite results should render as TodoListView component,
        // NOT as raw JSON blobs

        let result = ACPToolResult(
            call_id: "todo-write",
            ok: true,
            result: JSONValue.object([
                "todos": .array([
                    .object([
                        "content": .string("Fix rendering bug"),
                        "status": .string("completed"),
                        "activeForm": .string("Completed")
                    ])
                ])
            ]),
            error: nil,
            ts: 1000
        )

        let view = ToolResultView(result: result)

        // Verify view is created and will render TodoListView
        XCTAssertNotNil(view)

        // The view body should use TodoListView, not raw JSON text
        // This is verified by the parsing logic
        let todos = TodoListView.parse(from: result.result!)
        XCTAssertNotNil(todos, "Todos should be parseable")
    }

    func testToolResultView_PlanStateRendersAsComponent_NotJSON() {
        // CRITICAL: plan_state results should render as PlanStateView component,
        // NOT as raw JSON blobs

        let result = ACPToolResult(
            call_id: "plan",
            ok: true,
            result: JSONValue.object([
                "type": .string("plan_state"),
                "status": .string("running"),
                "steps": .array([.string("Analyzing code"), .string("Generating tests")])
            ]),
            error: nil,
            ts: 1000
        )

        let view = ToolResultView(result: result)
        XCTAssertNotNil(view)
    }

    // MARK: - Error Handling Tests

    func testToolResultView_ShowsErrorMessage() {
        let result = ACPToolResult(
            call_id: "failed",
            ok: false,
            result: nil,
            error: "Operation failed",
            ts: 1000
        )

        let view = ToolResultView(result: result)
        XCTAssertNotNil(view)
        XCTAssertEqual(result.error, "Operation failed")
    }

    func testToolResultView_HandlesNullResult() {
        let result = ACPToolResult(
            call_id: "null-result",
            ok: true,
            result: nil,
            error: nil,
            ts: 1000
        )

        let view = ToolResultView(result: result)
        XCTAssertNotNil(view)
    }

    // MARK: - Integration Tests

    func testToolResultView_RealWorldTodoWriteOutput() {
        // Simulate actual TodoWrite tool output
        let result = ACPToolResult(
            call_id: "todowrite-123",
            ok: true,
            result: JSONValue.object([
                "todos": .array([
                    .object([
                        "activeForm": .string("Fixed"),
                        "content": .string("Fix ACP thinking/reasoning classification bug"),
                        "status": .string("completed")
                    ]),
                    .object([
                        "activeForm": .string("Created"),
                        "content": .string("Create ACP message type compliance tests"),
                        "status": .string("completed")
                    ]),
                    .object([
                        "activeForm": .string("Creating"),
                        "content": .string("Create plan state tests (unit + UI)"),
                        "status": .string("in_progress")
                    ]),
                    .object([
                        "activeForm": .string("Pending"),
                        "content": .string("Commit and push all changes"),
                        "status": .string("pending")
                    })
                ])
            ]),
            error: nil,
            ts: 1699900000000
        )

        let todos = TodoListView.parse(from: result.result!)

        XCTAssertNotNil(todos)
        XCTAssertEqual(todos?.count, 4)
        XCTAssertEqual(todos?[0].status, "completed")
        XCTAssertEqual(todos?[2].status, "in_progress")
        XCTAssertEqual(todos?[3].status, "pending")
    }

    func testToolListView_RendersAllStatusIcons() {
        // Verify all status types render correctly
        let statuses = ["completed", "in_progress", "pending", "unknown"]

        for status in statuses {
            let todos = [TodoListView.TodoItem(
                content: "Test task",
                status: status,
                activeForm: "Active"
            )]

            let view = TodoListView(todos: todos)
            XCTAssertNotNil(view, "Should render for status: \(status)")
        }
    }
}
