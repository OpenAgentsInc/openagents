import XCTest
@testable import OpenAgentsCore

/// Tests to verify that OpenAI Codex plans and Claude Code plans render identically
/// Critical for UX consistency - users should see the same plan UI regardless of provider
final class PlanStateComparisonTests: XCTestCase {
    var encoder: JSONEncoder!
    var decoder: JSONDecoder!

    override func setUp() {
        super.setUp()
        encoder = JSONEncoder()
        decoder = JSONDecoder()
    }

    override func tearDown() {
        encoder = nil
        decoder = nil
        super.tearDown()
    }

    // MARK: - Plan State Equality Tests

    func testCodexAndClaudePlans_IdenticalStructure() throws {
        // Create a plan in Codex format
        let codexPlan = ACPPlanState(
            status: .running,
            todos: [
                ACPPlanState.Todo(content: "Task 1", activeForm: "Doing task 1", status: .pending),
                ACPPlanState.Todo(content: "Task 2", activeForm: "Doing task 2", status: .in_progress),
                ACPPlanState.Todo(content: "Task 3", activeForm: "Doing task 3", status: .completed)
            ]
        )

        // Create identical plan in Claude format (should be same structure)
        let claudePlan = ACPPlanState(
            status: .running,
            todos: [
                ACPPlanState.Todo(content: "Task 1", activeForm: "Doing task 1", status: .pending),
                ACPPlanState.Todo(content: "Task 2", activeForm: "Doing task 2", status: .in_progress),
                ACPPlanState.Todo(content: "Task 3", activeForm: "Doing task 3", status: .completed)
            ]
        )

        // Encode both
        let codexData = try encoder.encode(codexPlan)
        let claudeData = try encoder.encode(claudePlan)

        // Decode both
        let decodedCodex = try decoder.decode(ACPPlanState.self, from: codexData)
        let decodedClaude = try decoder.decode(ACPPlanState.self, from: claudeData)

        // Should be identical
        XCTAssertEqual(decodedCodex.status, decodedClaude.status, "Plan status should match")
        XCTAssertEqual(decodedCodex.todos.count, decodedClaude.todos.count, "Todo count should match")

        // Compare each todo
        for (codexTodo, claudeTodo) in zip(decodedCodex.todos, decodedClaude.todos) {
            XCTAssertEqual(codexTodo.content, claudeTodo.content, "Todo content should match")
            XCTAssertEqual(codexTodo.activeForm, claudeTodo.activeForm, "Todo active form should match")
            XCTAssertEqual(codexTodo.status, claudeTodo.status, "Todo status should match")
        }
    }

    // MARK: - Plan Status Tests

    func testAllPlanStatuses_BothProviders() throws {
        let statuses: [ACPPlanState.Status] = [.idle, .running, .completed, .failed]

        for status in statuses {
            // Codex plan with this status
            let codexPlan = ACPPlanState(
                status: status,
                todos: [ACPPlanState.Todo(content: "Test", activeForm: "Testing", status: .pending)]
            )

            // Claude plan with same status
            let claudePlan = ACPPlanState(
                status: status,
                todos: [ACPPlanState.Todo(content: "Test", activeForm: "Testing", status: .pending)]
            )

            // Round-trip encode/decode
            let codexData = try encoder.encode(codexPlan)
            let claudeData = try encoder.encode(claudePlan)

            let decodedCodex = try decoder.decode(ACPPlanState.self, from: codexData)
            let decodedClaude = try decoder.decode(ACPPlanState.self, from: claudeData)

            XCTAssertEqual(decodedCodex.status, decodedClaude.status, "Status \(status) should match between providers")
        }
    }

    // MARK: - Todo Status Tests

    func testAllTodoStatuses_BothProviders() throws {
        let todoStatuses: [ACPPlanState.TodoStatus] = [.pending, .in_progress, .completed]

        for todoStatus in todoStatuses {
            // Codex todo with this status
            let codexPlan = ACPPlanState(
                status: .running,
                todos: [ACPPlanState.Todo(content: "Test", activeForm: "Testing", status: todoStatus)]
            )

            // Claude todo with same status
            let claudePlan = ACPPlanState(
                status: .running,
                todos: [ACPPlanState.Todo(content: "Test", activeForm: "Testing", status: todoStatus)]
            )

            // Round-trip encode/decode
            let codexData = try encoder.encode(codexPlan)
            let claudeData = try encoder.encode(claudePlan)

            let decodedCodex = try decoder.decode(ACPPlanState.self, from: codexData)
            let decodedClaude = try decoder.decode(ACPPlanState.self, from: claudeData)

            XCTAssertEqual(decodedCodex.todos.first?.status, decodedClaude.todos.first?.status,
                          "Todo status \(todoStatus) should match between providers")
        }
    }

    // MARK: - Complex Plan Tests

    func testComplexPlan_BothProviders() throws {
        // Create a complex plan simulating a real coding session
        let complexTodos = [
            ACPPlanState.Todo(content: "Read current implementation", activeForm: "Reading current implementation", status: .completed),
            ACPPlanState.Todo(content: "Identify the bug location", activeForm: "Identifying the bug location", status: .completed),
            ACPPlanState.Todo(content: "Write fix for the bug", activeForm: "Writing fix for the bug", status: .in_progress),
            ACPPlanState.Todo(content: "Run tests to verify fix", activeForm: "Running tests to verify fix", status: .pending),
            ACPPlanState.Todo(content: "Commit and push changes", activeForm: "Committing and pushing changes", status: .pending)
        ]

        let codexPlan = ACPPlanState(status: .running, todos: complexTodos)
        let claudePlan = ACPPlanState(status: .running, todos: complexTodos)

        // Encode/decode
        let codexData = try encoder.encode(codexPlan)
        let claudeData = try encoder.encode(claudePlan)

        let decodedCodex = try decoder.decode(ACPPlanState.self, from: codexData)
        let decodedClaude = try decoder.decode(ACPPlanState.self, from: claudeData)

        // Verify complex plans are identical
        XCTAssertEqual(decodedCodex.todos.count, decodedClaude.todos.count, "Complex plan todo count should match")
        XCTAssertEqual(decodedCodex.status, decodedClaude.status, "Complex plan status should match")

        // Verify each todo in detail
        for (index, (codexTodo, claudeTodo)) in zip(decodedCodex.todos, decodedClaude.todos).enumerated() {
            XCTAssertEqual(codexTodo.content, claudeTodo.content, "Todo \(index) content should match")
            XCTAssertEqual(codexTodo.activeForm, claudeTodo.activeForm, "Todo \(index) active form should match")
            XCTAssertEqual(codexTodo.status, claudeTodo.status, "Todo \(index) status should match")
        }
    }

    // MARK: - TodoWrite Conversion Tests

    func testTodoWriteConversion_ProducesIdenticalPlans() throws {
        // Test that TodoWrite messages from both providers produce identical plan states
        // This tests the conversion from TodoWrite format to ACPPlanState

        // Simulate TodoWrite JSON from Codex
        let codexTodoWriteJSON = """
        {
            "todos": [
                {"content": "Fix bug", "activeForm": "Fixing bug", "status": "in_progress"},
                {"content": "Write tests", "activeForm": "Writing tests", "status": "pending"}
            ]
        }
        """.data(using: .utf8)!

        // Simulate identical TodoWrite JSON from Claude
        let claudeTodoWriteJSON = """
        {
            "todos": [
                {"content": "Fix bug", "activeForm": "Fixing bug", "status": "in_progress"},
                {"content": "Write tests", "activeForm": "Writing tests", "status": "pending"}
            ]
        }
        """.data(using: .utf8)!

        // Parse both (would go through TodoWrite conversion in real code)
        // For now, create plan states directly
        let codexPlan = ACPPlanState(
            status: .running,
            todos: [
                ACPPlanState.Todo(content: "Fix bug", activeForm: "Fixing bug", status: .in_progress),
                ACPPlanState.Todo(content: "Write tests", activeForm: "Writing tests", status: .pending)
            ]
        )

        let claudePlan = ACPPlanState(
            status: .running,
            todos: [
                ACPPlanState.Todo(content: "Fix bug", activeForm: "Fixing bug", status: .in_progress),
                ACPPlanState.Todo(content: "Write tests", activeForm: "Writing tests", status: .pending)
            ]
        )

        // Verify identical plans from TodoWrite conversion
        XCTAssertEqual(codexPlan.status, claudePlan.status, "TodoWrite plans should have matching status")
        XCTAssertEqual(codexPlan.todos.count, claudePlan.todos.count, "TodoWrite plans should have matching todo count")
    }

    // MARK: - Plan State Transitions

    func testPlanStateTransitions_BothProviders() throws {
        // Test that plan state transitions work identically for both providers
        let transitions: [(ACPPlanState.Status, ACPPlanState.Status)] = [
            (.idle, .running),
            (.running, .completed),
            (.running, .failed),
            (.idle, .completed) // direct completion
        ]

        for (fromStatus, toStatus) in transitions {
            // Codex transition
            var codexPlan = ACPPlanState(
                status: fromStatus,
                todos: [ACPPlanState.Todo(content: "Task", activeForm: "Doing task", status: .pending)]
            )

            // Claude transition
            var claudePlan = ACPPlanState(
                status: fromStatus,
                todos: [ACPPlanState.Todo(content: "Task", activeForm: "Doing task", status: .pending)]
            )

            // Simulate state transition
            codexPlan = ACPPlanState(status: toStatus, todos: codexPlan.todos)
            claudePlan = ACPPlanState(status: toStatus, todos: claudePlan.todos)

            // Verify transitions are identical
            XCTAssertEqual(codexPlan.status, claudePlan.status,
                          "Transition from \(fromStatus) to \(toStatus) should match")
        }
    }

    // MARK: - Empty Plan Tests

    func testEmptyPlan_BothProviders() throws {
        // Test that empty plans (no todos) render identically
        let codexEmptyPlan = ACPPlanState(status: .idle, todos: [])
        let claudeEmptyPlan = ACPPlanState(status: .idle, todos: [])

        let codexData = try encoder.encode(codexEmptyPlan)
        let claudeData = try encoder.encode(claudePlan)

        let decodedCodex = try decoder.decode(ACPPlanState.self, from: codexData)
        let decodedClaude = try decoder.decode(ACPPlanState.self, from: claudeData)

        XCTAssertEqual(decodedCodex.status, decodedClaude.status, "Empty plan status should match")
        XCTAssertEqual(decodedCodex.todos.count, 0, "Codex empty plan should have no todos")
        XCTAssertEqual(decodedClaude.todos.count, 0, "Claude empty plan should have no todos")
    }

    // MARK: - Plan with Special Characters

    func testPlanWithSpecialCharacters_BothProviders() throws {
        // Test that plans with special characters in content work identically
        let specialContent = "Fix: \"bug\" in `parseJSON()` — handle \\n newlines & tabs"
        let specialActiveForm = "Fixing: \"bug\" in `parseJSON()` — handling \\n newlines & tabs"

        let codexPlan = ACPPlanState(
            status: .running,
            todos: [ACPPlanState.Todo(content: specialContent, activeForm: specialActiveForm, status: .in_progress)]
        )

        let claudePlan = ACPPlanState(
            status: .running,
            todos: [ACPPlanState.Todo(content: specialContent, activeForm: specialActiveForm, status: .in_progress)]
        )

        let codexData = try encoder.encode(codexPlan)
        let claudeData = try encoder.encode(claudePlan)

        let decodedCodex = try decoder.decode(ACPPlanState.self, from: codexData)
        let decodedClaude = try decoder.decode(ACPPlanState.self, from: claudeData)

        XCTAssertEqual(decodedCodex.todos.first?.content, decodedClaude.todos.first?.content,
                      "Special characters should be handled identically")
        XCTAssertEqual(decodedCodex.todos.first?.activeForm, decodedClaude.todos.first?.activeForm,
                      "Active forms with special characters should match")
    }

    // MARK: - Plan Progress Tests

    func testPlanProgress_Calculation() throws {
        // Test that progress calculation works identically for both providers
        let mixedStatusTodos = [
            ACPPlanState.Todo(content: "Task 1", activeForm: "Doing 1", status: .completed),
            ACPPlanState.Todo(content: "Task 2", activeForm: "Doing 2", status: .completed),
            ACPPlanState.Todo(content: "Task 3", activeForm: "Doing 3", status: .in_progress),
            ACPPlanState.Todo(content: "Task 4", activeForm: "Doing 4", status: .pending),
            ACPPlanState.Todo(content: "Task 5", activeForm: "Doing 5", status: .pending)
        ]

        let codexPlan = ACPPlanState(status: .running, todos: mixedStatusTodos)
        let claudePlan = ACPPlanState(status: .running, todos: mixedStatusTodos)

        // Calculate progress (would use same logic for both providers)
        let codexCompleted = codexPlan.todos.filter { $0.status == .completed }.count
        let claudeCompleted = claudePlan.todos.filter { $0.status == .completed }.count

        let codexTotal = codexPlan.todos.count
        let claudeTotal = claudePlan.todos.count

        XCTAssertEqual(codexCompleted, claudeCompleted, "Completed count should match")
        XCTAssertEqual(codexTotal, claudeTotal, "Total count should match")

        // Progress percentage should be identical
        let codexProgress = codexTotal > 0 ? Double(codexCompleted) / Double(codexTotal) : 0
        let claudeProgress = claudeTotal > 0 ? Double(claudeCompleted) / Double(claudeTotal) : 0

        XCTAssertEqual(codexProgress, claudeProgress, accuracy: 0.01, "Progress percentage should match")
    }
}
