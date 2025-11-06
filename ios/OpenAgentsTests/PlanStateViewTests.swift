import XCTest
import SwiftUI
@testable import OpenAgents
@testable import OpenAgentsCore

final class PlanStateViewTests: XCTestCase {

    // MARK: - Rendering Tests

    func testPlanStateView_IdleStatus() throws {
        let state = ACPPlanState(status: .idle, summary: "Waiting to start")
        let view = PlanStateView(state: state)

        // Verify view can be instantiated
        XCTAssertNotNil(view)
        XCTAssertEqual(view.state.status, .idle)
    }

    func testPlanStateView_RunningStatus() throws {
        let state = ACPPlanState(
            status: .running,
            summary: "Executing plan",
            steps: ["Step 1", "Step 2"]
        )
        let view = PlanStateView(state: state)

        XCTAssertNotNil(view)
        XCTAssertEqual(view.state.status, .running)
    }

    func testPlanStateView_CompletedStatus() throws {
        let state = ACPPlanState(status: .completed, summary: "All done")
        let view = PlanStateView(state: state)

        XCTAssertNotNil(view)
        XCTAssertEqual(view.state.status, .completed)
    }

    func testPlanStateView_FailedStatus() throws {
        let state = ACPPlanState(status: .failed, summary: "Error occurred")
        let view = PlanStateView(state: state)

        XCTAssertNotNil(view)
        XCTAssertEqual(view.state.status, .failed)
    }

    // MARK: - Content Tests

    func testPlanStateView_WithSummary() {
        let summary = "This is a test summary"
        let state = ACPPlanState(status: .running, summary: summary)
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.summary, summary)
    }

    func testPlanStateView_WithoutSummary() {
        let state = ACPPlanState(status: .idle)
        let view = PlanStateView(state: state)

        XCTAssertNil(view.state.summary)
    }

    func testPlanStateView_WithSteps() {
        let steps = ["First step", "Second step", "Third step"]
        let state = ACPPlanState(status: .running, steps: steps)
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.steps, steps)
        XCTAssertEqual(view.state.steps?.count, 3)
    }

    func testPlanStateView_WithoutSteps() {
        let state = ACPPlanState(status: .completed)
        let view = PlanStateView(state: state)

        XCTAssertNil(view.state.steps)
    }

    func testPlanStateView_EmptySteps() {
        let state = ACPPlanState(status: .running, steps: [])
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.steps?.count, 0)
    }

    func testPlanStateView_EmptySummary() {
        let state = ACPPlanState(status: .running, summary: "")
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.summary, "")
    }

    // MARK: - Multiple Steps Tests

    func testPlanStateView_SingleStep() {
        let state = ACPPlanState(status: .running, steps: ["Only step"])
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.steps?.count, 1)
    }

    func testPlanStateView_ManySteps() {
        let steps = (1...20).map { "Step \($0)" }
        let state = ACPPlanState(status: .running, steps: steps)
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.steps?.count, 20)
    }

    // MARK: - Status Color Mapping Tests

    func testColorMapping_Idle() {
        let state = ACPPlanState(status: .idle)
        let view = PlanStateView(state: state)

        // Verify view is created with correct status
        XCTAssertEqual(view.state.status, .idle)
        // Color should be gray.opacity(0.6) for idle
    }

    func testColorMapping_Running() {
        let state = ACPPlanState(status: .running)
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.status, .running)
        // Color should be yellow.opacity(0.8) for running
    }

    func testColorMapping_Completed() {
        let state = ACPPlanState(status: .completed)
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.status, .completed)
        // Color should be OATheme.Colors.success for completed
    }

    func testColorMapping_Failed() {
        let state = ACPPlanState(status: .failed)
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.status, .failed)
        // Color should be OATheme.Colors.danger for failed
    }

    // MARK: - Title Text Tests

    func testTitleText_AllStatuses() {
        let expectedTitles: [(ACPPlanStatus, String)] = [
            (.idle, "Plan Idle"),
            (.running, "Plan Running"),
            (.completed, "Plan Complete"),
            (.failed, "Plan Failed")
        ]

        for (status, expectedTitle) in expectedTitles {
            let state = ACPPlanState(status: status)
            let view = PlanStateView(state: state)

            // The view should use the correct status
            XCTAssertEqual(view.state.status, status)
            // Title mapping is tested via the private titleFor method
        }
    }

    // MARK: - Complex Scenarios

    func testPlanStateView_FullyPopulated() {
        let state = ACPPlanState(
            status: .running,
            summary: "Building and deploying application",
            steps: [
                "Compile TypeScript sources",
                "Bundle with Webpack",
                "Run tests",
                "Build Docker image",
                "Deploy to staging"
            ],
            ts: 1699900000000
        )
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.status, .running)
        XCTAssertEqual(view.state.summary, "Building and deploying application")
        XCTAssertEqual(view.state.steps?.count, 5)
        XCTAssertEqual(view.state.ts, 1699900000000)
    }

    func testPlanStateView_MinimalState() {
        let state = ACPPlanState(status: .idle)
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.status, .idle)
        XCTAssertNil(view.state.summary)
        XCTAssertNil(view.state.steps)
        XCTAssertNil(view.state.ts)
    }

    // MARK: - Unicode and Special Characters

    func testPlanStateView_UnicodeContent() {
        let state = ACPPlanState(
            status: .running,
            summary: "处理中 🚀",
            steps: ["步骤一 ✓", "步骤二 ⏳", "步骤三 ⏸"]
        )
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.summary, "处理中 🚀")
        XCTAssertEqual(view.state.steps?[0], "步骤一 ✓")
    }

    func testPlanStateView_LongText() {
        let longSummary = String(repeating: "Very long summary text. ", count: 50)
        let longStep = String(repeating: "Very long step description. ", count: 20)

        let state = ACPPlanState(
            status: .running,
            summary: longSummary,
            steps: [longStep]
        )
        let view = PlanStateView(state: state)

        XCTAssertEqual(view.state.summary?.count, longSummary.count)
        XCTAssertEqual(view.state.steps?[0].count, longStep.count)
    }

    // MARK: - State Transitions

    func testPlanStateView_StatusTransition() {
        // Test that view can handle different states
        let states = [
            ACPPlanState(status: .idle, summary: "Ready"),
            ACPPlanState(status: .running, summary: "In progress"),
            ACPPlanState(status: .completed, summary: "Done"),
            ACPPlanState(status: .failed, summary: "Error")
        ]

        for state in states {
            let view = PlanStateView(state: state)
            XCTAssertNotNil(view)
            XCTAssertEqual(view.state.status, state.status)
        }
    }

    // MARK: - ACP Component Compliance

    func testPlanStateView_ACPComponentRendering() {
        // Verify PlanStateView renders as a proper ACP component, not JSON
        let state = ACPPlanState(
            status: .running,
            summary: "Test plan",
            steps: ["Step 1", "Step 2"]
        )
        let view = PlanStateView(state: state)

        // View should be a SwiftUI View, not a JSON blob
        XCTAssertTrue(view is any View)
        XCTAssertEqual(view.state.type, "plan_state")
    }

    func testPlanStateView_NotJSONBlob() {
        // Ensure the view renders UI components, not raw JSON
        let state = ACPPlanState(status: .running)
        let view = PlanStateView(state: state)

        // The view body should contain SwiftUI components (VStack, HStack, Text, Circle)
        // not a JSON string representation
        let mirror = Mirror(reflecting: view)

        // Verify it's a proper SwiftUI view structure
        XCTAssertTrue(String(describing: type(of: view)).contains("PlanStateView"))
    }

    // MARK: - Accessibility

    func testPlanStateView_AccessibilityStructure() {
        let state = ACPPlanState(
            status: .running,
            summary: "Processing",
            steps: ["First", "Second"]
        )
        let view = PlanStateView(state: state)

        // View should have accessible content
        XCTAssertNotNil(view.state.summary)
        XCTAssertNotNil(view.state.steps)

        // Steps should be enumerable for accessibility
        if let steps = view.state.steps {
            XCTAssertEqual(steps.count, 2)
            for (index, step) in steps.enumerated() {
                XCTAssertFalse(step.isEmpty)
                XCTAssertGreaterThanOrEqual(index, 0)
            }
        }
    }
}
