// Firm reason: 6 (XCUITest suite launching the native pure-black OpenAgents Cloud iOS app on Simulator and verifying Cloud shell, Account, Wallet, Environments & Projects, Usage, Sidebar, and Composer).
import XCTest

final class OpenAgentsUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private func composerElement(_ app: XCUIApplication) -> XCUIElement {
        app.textViews["chat.composer"].exists ? app.textViews["chat.composer"] : app.textFields["chat.composer"]
    }

    @MainActor
    func testPureBlackOpenAgentsCloudShellAccountWalletEnvironmentsUsageAndComposer() {
        let app = XCUIApplication()
        app.launchArguments = ["-OAResetStateForTests"]
        app.launch()

        // Top navigation bar & Cloud sign-in onboarding appear immediately.
        let sidebarButton = app.buttons["nav.sidebar"]
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["nav.computerPill"].exists)
        XCTAssertTrue(app.buttons["nav.workspace"].exists)
        XCTAssertTrue(app.buttons["nav.newChat"].exists)
        XCTAssertTrue(app.buttons["chat.signIn"].waitForExistence(timeout: 5))

        // Composer exposes Attachment (+), Model, Effort, Execution Target, Live Voice, Dictation, Full Auto & Usage HUD.
        XCTAssertTrue(app.buttons["chat.attachButton"].exists)
        XCTAssertTrue(app.buttons["chat.modelPicker"].exists)
        XCTAssertTrue(app.buttons["chat.effortPicker"].exists)
        XCTAssertTrue(app.buttons["chat.reviewMode"].exists)
        XCTAssertTrue(app.buttons["chat.liveVoiceButton"].exists)
        XCTAssertTrue(app.buttons["chat.micButton"].exists)
        XCTAssertTrue(app.buttons["chat.fullAuto"].exists)
        XCTAssertTrue(app.buttons["chat.usageBar"].exists)

        // Open Usage Dashboard sheet and dismiss cleanly.
        app.buttons["chat.usageBar"].tap()
        XCTAssertTrue(app.buttons["usage.done"].waitForExistence(timeout: 5))
        app.buttons["usage.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Open Environments & Projects sheet from top bar and verify zero LAN pairing fields.
        app.buttons["nav.computerPill"].tap()
        XCTAssertTrue(app.buttons["environments.done"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.textFields["computers.addAddress"].exists)
        XCTAssertFalse(app.buttons["computers.scanQR"].exists)
        app.buttons["environments.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Open sidebar and verify Cloud sections + Account, Wallet, and Environments footer buttons.
        sidebarButton.tap()
        XCTAssertTrue(app.buttons["sidebar.newChat"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["sidebar.dashboard"].exists)
        XCTAssertTrue(app.buttons["sidebar.account"].exists)
        XCTAssertTrue(app.buttons["sidebar.wallet"].exists)
        XCTAssertTrue(app.buttons["sidebar.computers"].exists)
        for secID in ["side.sec.1", "side.sec.2", "side.sec.3"] {
            XCTAssertTrue(
                app.otherElements[secID].exists || app.buttons[secID].exists || app.cells[secID].exists || app.staticTexts[secID].exists,
                "Expected Cloud sidebar section \(secID)"
            )
        }

        // Open Account sheet from sidebar and dismiss cleanly.
        app.buttons["sidebar.account"].tap()
        XCTAssertTrue(app.buttons["account.done"].waitForExistence(timeout: 5))
        app.buttons["account.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Verify composer accepts text natively.
        let composer = composerElement(app)
        XCTAssertTrue(composer.waitForExistence(timeout: 5))
        composer.tap()
        composer.typeText("/cloud")
        XCTAssertTrue(app.buttons["chat.send"].isEnabled)
    }
}
