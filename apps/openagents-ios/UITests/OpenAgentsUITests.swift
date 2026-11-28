// Firm reason: 6 (XCUITest suite launching the native pure-black OpenAgents Cloud iOS app on Simulator and verifying strict signed-out Login Gate, Email sign-in flow, signed-in Cloud shell, Account, Wallet, Environments & Projects, Usage, Sidebar, Composer, and Sign Out back to Login Gate).
import XCTest

final class OpenAgentsUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    private func composerElement(_ app: XCUIApplication) -> XCUIElement {
        app.textViews["chat.composer"].exists ? app.textViews["chat.composer"] : app.textFields["chat.composer"]
    }

    @MainActor
    func testSignedOutShowsStrictLoginGateOnly() {
        let app = XCUIApplication()
        app.launchArguments = ["-OAResetStateForTests"]
        app.launch()

        // Signed-out root shows ONLY the full-screen Login Gate (Google, Apple, Email).
        let googleBtn = app.buttons["login.google"]
        let appleBtn = app.buttons["login.apple"]
        let emailBtn = app.buttons["login.email"]
        XCTAssertTrue(googleBtn.waitForExistence(timeout: 15))
        XCTAssertTrue(appleBtn.exists)
        XCTAssertTrue(emailBtn.exists)

        // Zero GitHub login anywhere.
        XCTAssertFalse(app.buttons["Continue with GitHub"].exists)

        // Strict auth gate: ZERO chat, sidebar, composer, or usage HUD visible before sign-in.
        XCTAssertFalse(app.buttons["nav.sidebar"].exists)
        XCTAssertFalse(app.buttons["nav.computerPill"].exists)
        XCTAssertFalse(app.buttons["nav.newChat"].exists)
        XCTAssertFalse(app.buttons["chat.usageBar"].exists)
        XCTAssertFalse(app.buttons["chat.fullAuto"].exists)
        XCTAssertFalse(app.textViews["chat.composer"].exists)
        XCTAssertFalse(app.textFields["chat.composer"].exists)

        // Personal or work email opens the clean pure-black email code sign-in sheet and closes cleanly.
        emailBtn.tap()
        XCTAssertTrue(app.textFields["login.email.input"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["login.email.send"].exists)
        app.buttons["login.email.close"].tap()
        XCTAssertTrue(googleBtn.waitForExistence(timeout: 5))
    }

    @MainActor
    func testSignedInPureBlackOpenAgentsCloudShellAndSignOutReturnsToLoginGate() {
        let app = XCUIApplication()
        app.launchArguments = ["-OAResetStateForTests", "-OADemoPreview", "1"]
        app.launch()

        // Signed-in Cloud workspace renders Metal GPU top bar, Usage HUD, and Composer.
        let sidebarButton = app.buttons["nav.sidebar"]
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 15))
        XCTAssertTrue(app.buttons["nav.computerPill"].exists)
        XCTAssertTrue(app.buttons["nav.workspace"].exists)
        XCTAssertTrue(app.buttons["nav.newChat"].exists)
        XCTAssertFalse(app.buttons["login.google"].exists)

        // Composer exposes Attachment (+), bottom-left Execution Target chip, Live Voice, Dictation & Usage HUD.
        // Model picker, effort picker, and Full Auto mode toggle are omitted in v1.0.0 (parity with Web & Desktop).
        XCTAssertTrue(app.buttons["chat.attachButton"].exists)
        XCTAssertFalse(app.buttons["chat.modelPicker"].exists)
        XCTAssertFalse(app.buttons["chat.effortPicker"].exists)
        XCTAssertFalse(app.buttons["chat.fullAuto"].exists)
        XCTAssertTrue(app.buttons["chat.reviewMode"].exists)
        XCTAssertTrue(app.buttons["chat.liveVoiceButton"].exists)
        XCTAssertTrue(app.buttons["chat.micButton"].exists)
        XCTAssertTrue(app.buttons["chat.usageBar"].exists)

        // Open Usage & Credits sheet and verify no Full Auto Policy toggle exists, then dismiss cleanly.
        app.buttons["chat.usageBar"].tap()
        XCTAssertTrue(app.buttons["usage.done"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.switches["usage.fullAutoToggle"].exists)
        app.buttons["usage.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Open Environments & Projects sheet from top bar and verify zero LAN pairing fields.
        app.buttons["nav.computerPill"].tap()
        XCTAssertTrue(app.buttons["environments.done"].waitForExistence(timeout: 5))
        XCTAssertFalse(app.textFields["computers.addAddress"].exists)
        XCTAssertFalse(app.buttons["computers.scanQR"].exists)
        app.buttons["environments.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        // Open sidebar and verify Cloud sections (Pinned + Chats + Projects + Environments) + Account & Environments footer (NO Spark Wallet in v1.0.0).
        sidebarButton.tap()
        XCTAssertTrue(app.buttons["sidebar.newChat"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["sidebar.dashboard"].exists)
        XCTAssertTrue(app.buttons["sidebar.account"].exists)
        XCTAssertFalse(app.buttons["sidebar.wallet"].exists)
        XCTAssertTrue(app.buttons["sidebar.computers"].exists)
        for secID in ["side.sec.4", "side.sec.1", "side.sec.2", "side.sec.3"] {
            XCTAssertTrue(
                app.otherElements[secID].exists || app.buttons[secID].exists || app.cells[secID].exists || app.staticTexts[secID].exists,
                "Expected Cloud sidebar section \(secID)"
            )
        }

        // Open Account & Cloud sheet and verify GitHub status row + Done dismiss.
        app.buttons["sidebar.account"].tap()
        XCTAssertTrue(app.buttons["account.done"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.otherElements["account.github"].exists || app.staticTexts["GitHub"].exists)
        app.buttons["account.done"].tap()
        XCTAssertTrue(sidebarButton.waitForExistence(timeout: 5))

        let composer = composerElement(app)
        XCTAssertTrue(composer.waitForExistence(timeout: 5))
        composer.tap()
        composer.typeText("/cloud")
        XCTAssertTrue(app.buttons["chat.send"].isEnabled)

        // Signing out returns immediately to the strict full-screen Login Gate.
        sidebarButton.tap()
        XCTAssertTrue(app.buttons["sidebar.account"].waitForExistence(timeout: 5))
        app.buttons["sidebar.account"].tap()
        XCTAssertTrue(app.buttons["account.logout"].waitForExistence(timeout: 5))
        app.buttons["account.logout"].tap()
        XCTAssertTrue(app.buttons["login.google"].waitForExistence(timeout: 8))
        XCTAssertFalse(app.buttons["nav.sidebar"].exists)
    }
}
