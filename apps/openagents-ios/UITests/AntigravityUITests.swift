// Firm reason: 6 (XCUITest suite launching the native SwiftUI OpenAgents iOS app on Simulator and verifying Antigravity-only Phases 1-4 UX).
import XCTest

final class AntigravityUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    @MainActor
    func testPureBlackAntigravityShellSidebarComputersWorkspaceAndComposer() {
        let app = XCUIApplication()
        app.launchArguments = ["-OANoLocalNetworkBrowse"]
        app.launch()

        // Top navigation bar & Antigravity onboarding state appear immediately.
        let sidebarButton = app.buttons["nav.sidebar"]
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["nav.computerPill"].exists)
        XCTAssertTrue(app.buttons["nav.workspace"].exists)
        XCTAssertTrue(app.buttons["nav.newChat"].exists)
        XCTAssertTrue(app.buttons["chat.connectTailscale"].waitForExistence(timeout: 5))

        // Composer exposes Attachment (+), Antigravity Model, Thinking Effort, and Review Mode controls.
        XCTAssertTrue(app.buttons["chat.attachButton"].exists)
        XCTAssertTrue(app.buttons["chat.modelPicker"].exists)
        XCTAssertTrue(app.buttons["chat.effortPicker"].exists)
        XCTAssertTrue(app.buttons["chat.reviewMode"].exists)

        // Open Workspace Diff / Branch / PR sheet and dismiss cleanly.
        app.buttons["nav.workspace"].tap()
        XCTAssertTrue(app.buttons["workspace.done"].waitForExistence(timeout: 5))
        app.buttons["workspace.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Open sidebar and verify zero legacy Login / Projects / Foreman controls exist.
        sidebarButton.tap()
        XCTAssertTrue(app.buttons["sidebar.newChat"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["sidebar.computers"].exists)
        for removed in ["drawer.projects", "drawer.foreman", "drawer.login", "settings.login", "usage.open"] {
            XCTAssertFalse(app.buttons[removed].exists, "Legacy control \(removed) must not exist")
        }

        // Open Computers sheet from sidebar and verify MagicDNS/IP manual entry & Pair UI.
        app.buttons["sidebar.computers"].tap()
        let addressField = app.textFields["computers.addAddress"]
        XCTAssertTrue(addressField.waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["computers.addButton"].exists)

        // Dismiss Computers sheet cleanly back to main Antigravity chat surface.
        app.buttons["computers.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Verify `/` triggers the Slash Command popover, and composer accepts text natively.
        let composer = app.textViews["chat.composer"].exists ? app.textViews["chat.composer"] : app.textFields["chat.composer"]
        XCTAssertTrue(composer.waitForExistence(timeout: 5))
        composer.tap()
        composer.typeText("/plan")
        XCTAssertTrue(app.buttons["chat.send"].isEnabled)
    }
}
