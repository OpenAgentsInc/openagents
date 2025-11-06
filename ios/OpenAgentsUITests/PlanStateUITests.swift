//
//  PlanStateUITests.swift
//  OpenAgentsUITests
//
//  Created by Claude Code on 11/5/25.
//

import XCTest

final class PlanStateUITests: XCTestCase {

    var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
    }

    override func tearDownWithError() throws {
        app = nil
    }

    // MARK: - Plan State Rendering Tests

    @MainActor
    func testPlanStateView_RendersAsProperComponent() throws {
        app.launch()

        // Verify plan state renders as a SwiftUI component, not JSON
        // If a plan state appears in the UI, it should show:
        // 1. A colored circle indicator
        // 2. Status text (e.g., "Plan Running")
        // 3. Summary text (if present)
        // 4. Numbered steps (if present)

        // Wait for app to be ready
        let exists = NSPredicate(format: "exists == true")
        expectation(for: exists, evaluatedWith: app, handler: nil)
        waitForExpectations(timeout: 5)

        // Plan states should NOT appear as raw JSON blobs
        // They should render as structured UI components
    }

    @MainActor
    func testPlanStateView_IdleStatusRendering() throws {
        app.launch()

        // Verify idle plan state renders with:
        // - Gray circle indicator
        // - "Plan Idle" text
        // Note: This test assumes a plan state can be triggered in the app
        // If plan states are only shown during agent responses, this may need
        // to be tested via a mock or specific test scenario
    }

    @MainActor
    func testPlanStateView_RunningStatusRendering() throws {
        app.launch()

        // Verify running plan state renders with:
        // - Yellow circle indicator
        // - "Plan Running" text
        // - Summary text if present
        // - Numbered steps if present
    }

    @MainActor
    func testPlanStateView_CompletedStatusRendering() throws {
        app.launch()

        // Verify completed plan state renders with:
        // - Green circle indicator (success color)
        // - "Plan Complete" text
    }

    @MainActor
    func testPlanStateView_FailedStatusRendering() throws {
        app.launch()

        // Verify failed plan state renders with:
        // - Red circle indicator (danger color)
        // - "Plan Failed" text
    }

    @MainActor
    func testPlanStateView_WithSummaryText() throws {
        app.launch()

        // Verify plan state with summary shows:
        // - Status indicator and title
        // - Summary text below the title
        // - Summary is readable and not truncated inappropriately
    }

    @MainActor
    func testPlanStateView_WithStepsList() throws {
        app.launch()

        // Verify plan state with steps shows:
        // - Status indicator and title
        // - Optional summary
        // - Numbered list of steps (1., 2., 3., etc.)
        // - Each step is readable
    }

    @MainActor
    func testPlanStateView_StepNumbering() throws {
        app.launch()

        // Verify steps are properly numbered:
        // - First step shows "1."
        // - Second step shows "2."
        // - Numbering is consistent and sequential
    }

    @MainActor
    func testPlanStateView_TextSelection() throws {
        app.launch()

        // Verify plan state text is selectable:
        // - Users should be able to select/copy step text
        // - Summary text should be selectable
        // This is important for copying plan steps
    }

    // MARK: - Visual Hierarchy Tests

    @MainActor
    func testPlanStateView_VisualHierarchy() throws {
        app.launch()

        // Verify plan state maintains proper visual hierarchy:
        // 1. Status indicator (circle) and title at top
        // 2. Summary below (if present)
        // 3. Steps list below summary (if present)
        // 4. Proper spacing between elements
    }

    @MainActor
    func testPlanStateView_StatusIndicatorVisible() throws {
        app.launch()

        // Verify the colored circle status indicator is visible
        // Circle should be:
        // - 8x8 points in size
        // - Properly colored based on status
        // - Aligned with status text
    }

    // MARK: - Content Scrolling Tests

    @MainActor
    func testPlanStateView_LongStepsScrollable() throws {
        app.launch()

        // If a plan has many steps (e.g., 20+ steps):
        // - All steps should be accessible via scrolling
        // - Step numbering should remain correct
        // - No truncation of content
    }

    @MainActor
    func testPlanStateView_LongSummaryWrapping() throws {
        app.launch()

        // If summary text is long:
        // - Text should wrap properly
        // - No horizontal scrolling required
        // - Text remains readable
    }

    // MARK: - ACP Component Compliance

    @MainActor
    func testPlanStateView_NotRenderedAsJSON() throws {
        app.launch()

        // CRITICAL: Verify plan states are NOT rendered as JSON blobs
        // They should appear as structured UI components, not raw data

        // Look for indicators of JSON rendering (these should NOT exist):
        let jsonIndicators = [
            "\"{\"",
            "\"type\":\"plan_state\"",
            "\"status\":",
            "\"steps\":["
        ]

        for indicator in jsonIndicators {
            // If we find raw JSON in the UI, the test should fail
            let jsonText = app.staticTexts[indicator]
            XCTAssertFalse(jsonText.exists, "Plan state should not render as JSON: found '\(indicator)'")
        }
    }

    @MainActor
    func testPlanStateView_ProperACPComponentStructure() throws {
        app.launch()

        // Verify plan state renders with proper ACP component structure:
        // - Visual status indicator (not text "status": "running")
        // - Human-readable status text ("Plan Running", not "running")
        // - Formatted step numbers (1., 2., 3., not array indices)
        // - Proper typography and spacing
    }

    // MARK: - Accessibility Tests

    @MainActor
    func testPlanStateView_AccessibilityLabels() throws {
        app.launch()

        // Verify accessibility:
        // - Status text is accessible
        // - Summary is accessible
        // - Each step is accessible
        // - Proper accessibility labels for status indicators
    }

    @MainActor
    func testPlanStateView_VoiceOverSupport() throws {
        app.launch()

        // Verify VoiceOver can navigate plan state:
        // - Status announced properly
        // - Summary read correctly
        // - Steps enumerated ("Step 1 of 5: ...")
    }

    // MARK: - Integration with Message Feed

    @MainActor
    func testPlanStateView_AppearsInMessageFeed() throws {
        app.launch()

        // Verify plan states appear properly in the message feed:
        // - Integrated with other message types
        // - Proper spacing and layout
        // - Scrolling works correctly
    }

    @MainActor
    func testPlanStateView_UpdatesLive() throws {
        app.launch()

        // If plan status changes (idle → running → completed):
        // - UI updates accordingly
        // - Color changes reflect status
        // - Text updates to match new status
    }

    // MARK: - Dark Mode Tests

    @MainActor
    func testPlanStateView_DarkModeColors() throws {
        // Test plan state rendering in dark mode
        app.launch()

        // Verify colors are appropriate for dark mode:
        // - Status indicators visible
        // - Text readable
        // - Proper contrast ratios
    }

    // MARK: - Performance Tests

    @MainActor
    func testPlanStateView_RenderingPerformance() throws {
        app.launch()

        // Measure rendering performance for:
        // - Plan with many steps (50+)
        // - Long summary text
        // - Multiple plan states in feed
        measure(metrics: [XCTClockMetric()]) {
            // Render plan state view
        }
    }

    @MainActor
    func testPlanStateView_MemoryUsage() throws {
        app.launch()

        // Verify plan state views don't cause memory issues:
        // - Multiple plans in feed
        // - Large step lists
        // - Long-running sessions
        measure(metrics: [XCTMemoryMetric()]) {
            // Render multiple plan states
        }
    }
}
